use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use op_backend::{BackendError, Op, Overlay, RevisionId, Snapshot};
use op_task::config::{self, FIRST_VERSION};
use op_task::layout;
use semver::Version;

use crate::TrackerError;

pub type Step = fn(&dyn Snapshot) -> Result<Vec<Op>, BackendError>;

#[derive(Clone)]
pub struct StoreVersion {
    pub version: Version,
    // The first release that reads this store version. `None` marks a store version that only
    // canary and source builds know yet.
    pub released: Option<&'static str>,
    // Rewrites a store of the version before this one into this one; `None` for the oldest.
    pub migrate: Option<Step>,
}

// A store version that this binary no longer migrates, and the last release that still does.
#[derive(Clone)]
pub struct Retired {
    pub version: Version,
    pub last_release: &'static str,
}

pub struct StoreVersions {
    // Oldest first.
    pub known: &'static [StoreVersion],
    pub retired: &'static [Retired],
}

// `mise run release` stamps its version on each store version that no release reads yet.
pub static STORE_VERSIONS: StoreVersions = StoreVersions {
    known: &[StoreVersion {
        version: FIRST_VERSION,
        released: Some("0.0.1"),
        migrate: None,
    }],
    retired: &[],
};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VersionError {
    #[error(
        "these tasks use store version {stored}, and this openplan reads store versions up to \
         {readable}; they need a newer openplan"
    )]
    Newer { stored: Version, readable: Version },
    #[error(
        "these tasks use store version {stored}, and this openplan migrates only store version \
         {oldest} and newer; {}",
        match last_release {
            Some(release) => format!(
                "run openplan {release} on this project once to migrate them, then update"
            ),
            None => "an older openplan must migrate them first".to_owned(),
        }
    )]
    Retired {
        stored: Version,
        oldest: Version,
        last_release: Option<&'static str>,
    },
    #[error(
        "these tasks use store version {stored}, and this build writes store version {current}, \
         which no release reads yet, so it does not migrate them by itself; run `openplan \
         migrate` to migrate them, and teammates then need a canary build until a release reads \
         store version {current}"
    )]
    Unmigrated { stored: Version, current: Version },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stored {
    Current,
    Older(Version),
}

impl StoreVersions {
    pub fn current(&self) -> &Version {
        &self.newest().version
    }

    pub fn oldest(&self) -> &Version {
        &self
            .known
            .first()
            .expect("a binary knows one store version")
            .version
    }

    // A daemon migrates a store by itself only to a store version that a stable release reads, so a
    // canary never moves a team's store out of reach of their releases.
    pub fn migrates_by_itself(&self) -> bool {
        self.newest().released.is_some()
    }

    pub fn stored(&self, stored: &Version) -> Result<Stored, VersionError> {
        let current = self.current();
        if stored > current {
            return Err(VersionError::Newer {
                stored: stored.clone(),
                readable: current.clone(),
            });
        }
        if stored < self.oldest() {
            return Err(VersionError::Retired {
                stored: stored.clone(),
                oldest: self.oldest().clone(),
                last_release: self
                    .retired
                    .iter()
                    .find(|retired| retired.version == *stored)
                    .map(|retired| retired.last_release),
            });
        }
        Ok(match stored == current {
            true => Stored::Current,
            false => Stored::Older(stored.clone()),
        })
    }

    pub fn unmigrated(&self, stored: &Version) -> VersionError {
        VersionError::Unmigrated {
            stored: stored.clone(),
            current: self.current().clone(),
        }
    }

    // The writes that move a store of version `from` to version `to`, its `version` key included.
    pub fn migration(
        &self,
        snapshot: &dyn Snapshot,
        from: &Version,
        to: &Version,
    ) -> Result<Vec<Op>, TrackerError> {
        let mut overlay = Overlay::new(snapshot);
        for known in self
            .known
            .iter()
            .filter(|known| known.version > *from && known.version <= *to)
        {
            let step = known
                .migrate
                .expect("every store version after the oldest migrates from the one before it");
            let ops = step(&overlay)?;
            overlay.apply(ops);
        }
        if let Some(text) = overlay.read_text(layout::CONFIG)? {
            let stamped = config::restamp(&text, to)?;
            overlay.set(layout::CONFIG, Some(stamped.into_bytes()));
        }
        Ok(overlay.into_ops())
    }

    // The store as this binary reads it: a store of an older version reads as if migrated, and
    // `migrated_from` says from which. A store this binary cannot read stays as it is.
    pub fn view(&self, snapshot: Arc<dyn Snapshot>) -> Result<View, TrackerError> {
        let stored = version_of(&*snapshot)?.map(|version| self.stored(&version));
        match stored {
            Some(Ok(Stored::Older(version))) => {
                let ops = self.migration(&*snapshot, &version, self.current())?;
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

    fn newest(&self) -> &StoreVersion {
        self.known.last().expect("a binary knows one store version")
    }
}

pub struct View {
    pub snapshot: Arc<dyn Snapshot>,
    pub migrated_from: Option<Version>,
    pub problem: Option<VersionError>,
}

// A config that does not parse has no version; the config error says why.
pub(crate) fn version_of(snapshot: &dyn Snapshot) -> Result<Option<Version>, BackendError> {
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
