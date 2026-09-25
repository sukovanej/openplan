use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use op_backend::{BackendError, RevisionId, Snapshot};
use rusqlite::Connection;

use crate::history::{hash, storage};

// A larger document, such as an image a task embeds, stays in the history until someone reads it.
pub(crate) const HELD_BYTES: usize = 64 * 1024;

const BUSY: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
pub(crate) enum Stored {
    Held(Arc<[u8]>),
    InHistory { hash: String },
}

impl Stored {
    pub fn of(bytes: Vec<u8>) -> Self {
        match bytes.len() <= HELD_BYTES {
            true => Self::Held(bytes.into()),
            false => Self::InHistory { hash: hash(&bytes) },
        }
    }
}

// Its own connection, so a reader never waits for the lock a write holds.
pub(crate) struct Blobs(Mutex<Connection>);

impl Blobs {
    pub fn open(path: &Path) -> Result<Self, BackendError> {
        let db = Connection::open(path).map_err(storage)?;
        db.busy_timeout(BUSY).map_err(storage)?;
        Ok(Self(Mutex::new(db)))
    }

    fn read(&self, hash: &str) -> Result<Vec<u8>, BackendError> {
        self.0
            .lock()
            .expect("blob reader mutex poisoned")
            .query_row("SELECT bytes FROM blobs WHERE hash = ?1", [hash], |row| {
                row.get(0)
            })
            .map_err(storage)
    }
}

#[derive(Clone)]
pub(crate) struct LocalSnapshot {
    revision: Option<RevisionId>,
    files: BTreeMap<String, Stored>,
    blobs: Arc<Blobs>,
}

impl LocalSnapshot {
    pub fn new(
        revision: Option<RevisionId>,
        files: BTreeMap<String, Stored>,
        blobs: Arc<Blobs>,
    ) -> Self {
        Self {
            revision,
            files,
            blobs,
        }
    }

    // Shares every document the writes leave alone.
    pub fn with(
        &self,
        revision: RevisionId,
        writes: impl IntoIterator<Item = (String, Option<Stored>)>,
    ) -> Self {
        let mut files = self.files.clone();
        for (path, stored) in writes {
            match stored {
                Some(stored) => files.insert(path, stored),
                None => files.remove(&path),
            };
        }
        Self::new(Some(revision), files, Arc::clone(&self.blobs))
    }

    pub fn contains(&self, path: &str) -> bool {
        self.files.contains_key(path)
    }

    pub fn holds(&self, path: &str, bytes: &[u8]) -> bool {
        match self.files.get(path) {
            None => false,
            Some(Stored::Held(held)) => **held == *bytes,
            Some(Stored::InHistory { hash: held }) => hash(bytes) == *held,
        }
    }
}

impl Snapshot for LocalSnapshot {
    fn revision(&self) -> Option<&RevisionId> {
        self.revision.as_ref()
    }

    fn read(&self, path: &str) -> Result<Option<Vec<u8>>, BackendError> {
        match self.files.get(path) {
            None => Ok(None),
            Some(Stored::Held(held)) => Ok(Some(held.to_vec())),
            Some(Stored::InHistory { hash }) => self.blobs.read(hash).map(Some),
        }
    }

    fn files(&self) -> Result<Vec<String>, BackendError> {
        Ok(self.files.keys().cloned().collect())
    }
}
