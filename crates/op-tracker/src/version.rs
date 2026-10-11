use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use op_backend::{BackendError, Op, Overlay, RevisionId, Snapshot};
use op_task::config::{self, FIRST_VERSION};
use op_task::layout;

use crate::TrackerError;

pub type Step = fn(&dyn Snapshot) -> Result<Vec<Op>, BackendError>;

#[derive(Clone, Copy)]
pub struct StoreVersion {
    pub number: u32,
    // The first release that reads this store version. `None` marks a store version that only
    // canary and source builds know yet.
    pub released: Option<&'static str>,
    // `None` for the oldest store version, which has no version before it.
    pub migrate: Option<Step>,
}

#[derive(Clone, Copy)]
pub struct Retired {
    pub number: u32,
    pub last_release: &'static str,
}

pub struct StoreVersions {
    pub known: &'static [StoreVersion],
    pub retired: &'static [Retired],
}

// `mise run release` stamps its version on each store version that no release reads yet.
pub static STORE_VERSIONS: StoreVersions = StoreVersions {
    known: &[
        StoreVersion {
            number: FIRST_VERSION,
            released: Some("0.0.1"),
            migrate: None,
        },
        // Every release reads it, because a release from before the store versions ignores the
        // `version` key.
        StoreVersion {
            number: 2,
            released: Some("0.0.1"),
            migrate: Some(with_the_version_key),
        },
        StoreVersion {
            number: 3,
            released: Some("0.0.8"),
            migrate: Some(with_the_project_code_key),
        },
    ],
    retired: &[],
};

// The migration itself writes the `version` key, and that key is all that store version 2 adds.
fn with_the_version_key(_: &dyn Snapshot) -> Result<Vec<Op>, BackendError> {
    Ok(Vec::new())
}

// Store version 3 calls the letters of the task keys the project code.
fn with_the_project_code_key(snapshot: &dyn Snapshot) -> Result<Vec<Op>, BackendError> {
    let Some(text) = snapshot.read_text(layout::CONFIG)? else {
        return Ok(Vec::new());
    };
    let renamed =
        config::renamed(&text, "abbreviation", "project_code").map_err(BackendError::storage)?;
    Ok(vec![Op::put(layout::CONFIG, renamed)])
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VersionError {
    #[error(
        "these tasks use store version {stored}, and this openplan reads store versions up to \
         {readable}. They need a newer openplan"
    )]
    Newer { stored: u32, readable: u32 },
    #[error(
        "these tasks use store version {stored}, and this openplan migrates only store version \
         {oldest} and newer. {}",
        match last_release {
            Some(release) => format!(
                "Run openplan {release} on this project once to migrate them, then update"
            ),
            None => "An older openplan must migrate them first".to_owned(),
        }
    )]
    Retired {
        stored: u32,
        oldest: u32,
        last_release: Option<&'static str>,
    },
    #[error(
        "these tasks use store version {stored}, and this build writes store version {current}, \
         which no release reads yet. Run `openplan migrate` to migrate the tasks. Teammates then \
         need a canary build until a release reads store version {current}"
    )]
    Unmigrated { stored: u32, current: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stored {
    Current,
    Older(u32),
}

impl StoreVersions {
    pub fn current(&self) -> u32 {
        self.newest().number
    }

    pub fn oldest(&self) -> u32 {
        self.known
            .first()
            .expect("a binary knows one store version")
            .number
    }

    // A daemon migrates a store by itself only when a stable release reads the result, so a canary
    // never moves a team's store out of reach of their releases.
    pub fn migrates_by_itself(&self) -> bool {
        self.newest().released.is_some()
    }

    pub fn stored(&self, stored: u32) -> Result<Stored, VersionError> {
        let current = self.current();
        if stored > current {
            return Err(VersionError::Newer {
                stored,
                readable: current,
            });
        }
        if stored < self.oldest() {
            return Err(self.retired(stored));
        }
        Ok(match stored == current {
            true => Stored::Current,
            false => Stored::Older(stored),
        })
    }

    pub fn unmigrated(&self, stored: u32) -> VersionError {
        VersionError::Unmigrated {
            stored,
            current: self.current(),
        }
    }

    // The `version` key is part of the writes.
    pub fn migration(
        &self,
        snapshot: &dyn Snapshot,
        from: u32,
        to: u32,
    ) -> Result<Vec<Op>, TrackerError> {
        let mut overlay = Overlay::new(snapshot);
        for known in self
            .known
            .iter()
            .filter(|known| known.number > from && known.number <= to)
        {
            let step = known.migrate.ok_or_else(|| self.retired(from))?;
            let ops = step(&overlay)?;
            overlay.apply(ops);
        }
        if let Some(text) = overlay.read_text(layout::CONFIG)? {
            let stamped = config::restamp(&text, to)?;
            overlay.set(layout::CONFIG, Some(stamped.into_bytes()));
        }
        Ok(overlay.into_ops())
    }

    // A store of an older version reads as if migrated, and `migrated_from` says from which. A
    // store this binary cannot read stays as it is.
    pub fn view(&self, snapshot: Arc<dyn Snapshot>) -> Result<View, TrackerError> {
        let stored = version_of(&*snapshot)?.map(|version| self.stored(version));
        match stored {
            Some(Ok(Stored::Older(version))) => {
                let ops = self.migration(&*snapshot, version, self.current())?;
                Ok(View {
                    snapshot: Arc::new(Migrated::new(snapshot, ops)),
                    migrated_from: Some(version),
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

    fn retired(&self, stored: u32) -> VersionError {
        VersionError::Retired {
            stored,
            oldest: self.oldest(),
            last_release: self
                .retired
                .iter()
                .find(|retired| retired.number == stored)
                .map(|retired| retired.last_release),
        }
    }

    fn newest(&self) -> &StoreVersion {
        self.known.last().expect("a binary knows one store version")
    }
}

pub struct View {
    pub snapshot: Arc<dyn Snapshot>,
    pub migrated_from: Option<u32>,
    pub problem: Option<VersionError>,
}

// A config that does not parse has no version; the config error says why.
pub(crate) fn version_of(snapshot: &dyn Snapshot) -> Result<Option<u32>, BackendError> {
    Ok(snapshot
        .read_text(layout::CONFIG)?
        .and_then(|text| config::version(&text).ok()))
}

// A store of an older version with its migration applied in memory. It keeps the revision of the
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
