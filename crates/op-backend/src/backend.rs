use std::sync::Arc;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::{Actor, BackendError, Change, Op, Revision, RevisionId, Snapshot, diff};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Edit {
    pub message: String,
    pub ops: Vec<Op>,
}

impl Edit {
    pub fn new(message: impl Into<String>, ops: Vec<Op>) -> Self {
        Self {
            message: message.into(),
            ops,
        }
    }

    pub fn nothing() -> Self {
        Self::default()
    }
}

// The backend may call it more than once: a head that moves under a write makes the backend run it
// again against the new head, so it must derive everything from the snapshot it is given.
pub type Write<'a> = &'a mut dyn FnMut(Arc<dyn Snapshot>) -> Result<Edit, BackendError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Committed {
    pub revision: Revision,
    pub changes: Vec<Change>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LogQuery {
    pub prefix: String,
    pub before: Option<RevisionId>,
    pub limit: Option<usize>,
}

impl LogQuery {
    pub fn under(prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    pub revision: Revision,
    pub changes: Vec<Change>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    Local,
    Remote,
    External,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadMoved {
    pub from: Option<RevisionId>,
    pub to: Revision,
    pub changes: Vec<Change>,
    pub origin: Origin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendEvent {
    HeadMoved(HeadMoved),
    Sync(SyncStatus),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncStatus {
    pub remote: String,
    pub last_attempt: Option<Timestamp>,
    pub last_success: Option<Timestamp>,
    pub ahead: usize,
    pub behind: usize,
    pub error: Option<String>,
    pub syncing: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncReport {
    pub received: usize,
    pub sent: usize,
    pub merged: bool,
    pub changes: Vec<Change>,
}

pub trait Remote: Send + Sync {
    fn sync(&self) -> Result<SyncReport, BackendError>;

    fn status(&self) -> SyncStatus;
}

pub trait Backend: Send + Sync {
    fn head(&self) -> Result<Arc<dyn Snapshot>, BackendError>;

    fn at(&self, revision: &RevisionId) -> Result<Arc<dyn Snapshot>, BackendError>;

    // A backend that can find one document without the rest of the revision overrides this: a
    // history page reads two versions of each document it describes.
    fn read_at(&self, revision: &RevisionId, path: &str) -> Result<Option<Vec<u8>>, BackendError> {
        self.at(revision)?.read(path)
    }

    fn commit(&self, author: &Actor, write: Write<'_>) -> Result<Option<Committed>, BackendError>;

    // Newest first. A merge lists only the documents it wrote itself, such as a resolved conflict:
    // a document it took unchanged from one side is listed under the revision that made it.
    fn log(&self, query: &LogQuery) -> Result<Vec<LogEntry>, BackendError>;

    fn changes(
        &self,
        from: Option<&RevisionId>,
        to: &RevisionId,
    ) -> Result<Vec<Change>, BackendError> {
        let to = self.at(to)?;
        match from {
            Some(from) => diff(&*self.at(from)?, &*to),
            None => diff(&crate::MemorySnapshot::default(), &*to),
        }
    }

    fn refresh(&self) -> Result<Option<HeadMoved>, BackendError>;

    fn remote(&self) -> Option<&dyn Remote>;

    fn subscribe(&self) -> broadcast::Receiver<BackendEvent>;
}

pub trait BackendExt: Backend {
    fn transact<T, E>(
        &self,
        author: &Actor,
        mut write: impl FnMut(Arc<dyn Snapshot>) -> Result<(Edit, T), E>,
    ) -> Result<(Option<Committed>, T), E>
    where
        E: From<BackendError>,
    {
        let mut outcome: Option<Result<T, E>> = None;
        let committed = self.commit(author, &mut |snapshot| match write(snapshot) {
            Ok((edit, value)) => {
                outcome = Some(Ok(value));
                Ok(edit)
            }
            Err(err) => {
                outcome = Some(Err(err));
                Err(BackendError::Aborted)
            }
        });
        match (committed, outcome) {
            (Ok(committed), Some(Ok(value))) => Ok((committed, value)),
            (Err(BackendError::Aborted), Some(Err(err))) => Err(err),
            (Err(err), _) => Err(err.into()),
            (Ok(_), _) => Err(BackendError::Storage(
                "the backend committed without running the write".to_owned(),
            )
            .into()),
        }
    }
}

impl<B: Backend + ?Sized> BackendExt for B {}
