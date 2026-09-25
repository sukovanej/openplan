use crate::RevisionId;

#[derive(Debug, thiserror::Error)]
pub enum BackendError {
    #[error("not a document path: {0:?}")]
    InvalidPath(String),
    #[error("{0} is not UTF-8 text")]
    NotText(String),
    #[error("no such revision: {0}")]
    UnknownRevision(RevisionId),
    #[error("the write was abandoned")]
    Aborted,
    #[error("another writer kept moving the head; the write was not applied")]
    Contended,
    #[error("{0}")]
    Storage(String),
    #[error("sync: {0}")]
    Sync(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl BackendError {
    pub fn storage(err: impl std::fmt::Display) -> Self {
        Self::Storage(err.to_string())
    }

    pub fn sync(err: impl std::fmt::Display) -> Self {
        Self::Sync(err.to_string())
    }
}
