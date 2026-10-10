use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use op_backend::{BackendError, Op, Overlay, RevisionId, Snapshot};
use op_task::config::{self, Header};
use op_task::layout;

use crate::TrackerError;

pub type Step = fn(&dyn Snapshot) -> Result<Vec<Op>, BackendError>;

#[derive(Clone, Copy)]
pub struct Format {
    pub number: u32,
    // The first release that reads the format. `None` marks a format that only canary and source
    // builds know yet.
    pub released: Option<&'static str>,
    // Rewrites a store of the format before this one into this one; `None` for the oldest format.
    pub migrate: Option<Step>,
}

// A format that this binary no longer migrates, and the last release that still does.
#[derive(Clone, Copy)]
pub struct Retired {
    pub number: u32,
    pub last_release: &'static str,
}

pub struct Formats {
    // Oldest first.
    pub known: &'static [Format],
    pub retired: &'static [Retired],
}

// `mise run release` stamps its version on each format that no release reads yet.
pub static FORMATS: Formats = Formats {
    known: &[Format {
        number: 1,
        released: Some("0.0.1"),
        migrate: None,
    }],
    retired: &[],
};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FormatError {
    #[error(
        "these tasks use store format {format}, and this openplan reads formats up to {readable}; \
         {}",
        match requires {
            Some(version) => format!("they need openplan {version} or newer"),
            None => "they need a newer openplan".to_owned(),
        }
    )]
    Newer {
        format: u32,
        readable: u32,
        requires: Option<String>,
    },
    #[error(
        "these tasks use store format {format}, and this openplan migrates only format {oldest} \
         and newer; {}",
        match last_release {
            Some(release) => format!(
                "run openplan {release} on this project once to migrate them, then update"
            ),
            None => "an older openplan must migrate them first".to_owned(),
        }
    )]
    Retired {
        format: u32,
        oldest: u32,
        last_release: Option<&'static str>,
    },
    #[error(
        "these tasks use store format {format}, and this build writes format {current}, which no \
         release reads yet, so it does not migrate them by itself; run `openplan migrate` to \
         migrate them, and teammates then need openplan {requires} or newer"
    )]
    Unmigrated {
        format: u32,
        current: u32,
        requires: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stored {
    Current,
    Older(u32),
}

impl Formats {
    pub fn current(&self) -> u32 {
        self.newest().number
    }

    pub fn oldest(&self) -> u32 {
        self.known
            .first()
            .expect("a binary knows one format")
            .number
    }

    // A daemon migrates a store by itself only to a format that a stable release reads, so a canary
    // never moves a team's store out of reach of their releases.
    pub fn migrates_by_itself(&self) -> bool {
        self.newest().released.is_some()
    }

    pub fn header(&self) -> Header {
        self.header_of(self.current())
    }

    pub fn stored(&self, header: &Header) -> Result<Stored, FormatError> {
        let current = self.current();
        if header.format > current {
            return Err(FormatError::Newer {
                format: header.format,
                readable: current,
                requires: header.requires.clone(),
            });
        }
        if header.format < self.oldest() {
            return Err(FormatError::Retired {
                format: header.format,
                oldest: self.oldest(),
                last_release: self
                    .retired
                    .iter()
                    .find(|retired| retired.number == header.format)
                    .map(|retired| retired.last_release),
            });
        }
        Ok(match header.format == current {
            true => Stored::Current,
            false => Stored::Older(header.format),
        })
    }

    pub fn unmigrated(&self, format: u32) -> FormatError {
        FormatError::Unmigrated {
            format,
            current: self.current(),
            requires: self.requires_of(self.current()),
        }
    }

    // The writes that move a store of format `from` to format `to`, header included.
    pub fn migration(
        &self,
        snapshot: &dyn Snapshot,
        from: u32,
        to: u32,
    ) -> Result<Vec<Op>, TrackerError> {
        let mut overlay = Overlay::new(snapshot);
        for format in self
            .known
            .iter()
            .filter(|format| format.number > from && format.number <= to)
        {
            let step = format
                .migrate
                .expect("every format after the oldest migrates from the one before it");
            let ops = step(&overlay)?;
            overlay.apply(ops);
        }
        if let Some(text) = overlay.read_text(layout::CONFIG)? {
            let stamped = config::restamp(&text, &self.header_of(to))?;
            overlay.set(layout::CONFIG, Some(stamped.into_bytes()));
        }
        Ok(overlay.into_ops())
    }

    // The store as this binary reads it: a store of an older format reads as if migrated, and
    // `migrated_from` says from which. A store this binary cannot read stays as it is.
    pub fn view(&self, snapshot: Arc<dyn Snapshot>) -> Result<View, TrackerError> {
        let stored = header_of(&*snapshot)?.map(|header| self.stored(&header));
        match stored {
            Some(Ok(Stored::Older(format))) => {
                let ops = self.migration(&*snapshot, format, self.current())?;
                Ok(View {
                    snapshot: Arc::new(Migrated::new(snapshot, ops)),
                    migrated_from: Some(format),
                    problem: None,
                })
            }
            Some(Err(problem)) => Ok(View {
                snapshot,
                migrated_from: None,
                problem: Some(problem),
            }),
            Some(Ok(Stored::Current)) | None => Ok(View {
                snapshot,
                migrated_from: None,
                problem: None,
            }),
        }
    }

    fn newest(&self) -> &Format {
        self.known.last().expect("a binary knows one format")
    }

    fn header_of(&self, format: u32) -> Header {
        Header {
            format,
            requires: Some(self.requires_of(format)),
        }
    }

    // The release that first reads the format, or this build for a format no release reads yet.
    fn requires_of(&self, format: u32) -> String {
        self.known
            .iter()
            .find(|known| known.number == format)
            .and_then(|known| known.released)
            .unwrap_or(env!("CARGO_PKG_VERSION"))
            .to_owned()
    }
}

pub struct View {
    pub snapshot: Arc<dyn Snapshot>,
    pub migrated_from: Option<u32>,
    pub problem: Option<FormatError>,
}

// A config that does not parse has no header; the config error says why.
pub(crate) fn header_of(snapshot: &dyn Snapshot) -> Result<Option<Header>, BackendError> {
    Ok(snapshot
        .read_text(layout::CONFIG)?
        .and_then(|text| Header::parse(&text).ok()))
}

// A store of an older format with its migration applied in memory. It keeps the revision of the
// store it reads, so a reader can tell where it stands.
struct Migrated {
    base: Arc<dyn Snapshot>,
    edits: BTreeMap<String, Option<Vec<u8>>>,
}

impl Migrated {
    fn new(base: Arc<dyn Snapshot>, ops: Vec<Op>) -> Self {
        let edits = ops
            .into_iter()
            .map(|op| match op {
                Op::Put { path, bytes } => (path, Some(bytes)),
                Op::Remove { path } => (path, None),
            })
            .collect();
        Self { base, edits }
    }
}

impl Snapshot for Migrated {
    fn revision(&self) -> Option<&RevisionId> {
        self.base.revision()
    }

    fn read(&self, path: &str) -> Result<Option<Vec<u8>>, BackendError> {
        match self.edits.get(path) {
            Some(content) => Ok(content.clone()),
            None => self.base.read(path),
        }
    }

    fn files(&self) -> Result<Vec<String>, BackendError> {
        let mut files: BTreeSet<String> = self.base.files()?.into_iter().collect();
        for (path, content) in &self.edits {
            match content {
                Some(_) => files.insert(path.clone()),
                None => files.remove(path),
            };
        }
        Ok(files.into_iter().collect())
    }
}
