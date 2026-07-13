use std::path::Path;

#[derive(Debug, Clone)]
pub struct Repo {
    inner: gix::Repository,
}

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("could not open git repository: {0}")]
    Open(String),
}

impl Repo {
    pub fn discover(path: impl AsRef<Path>) -> Result<Self, GitError> {
        let inner = gix::discover(path.as_ref()).map_err(|e| GitError::Open(e.to_string()))?;
        Ok(Self { inner })
    }

    pub fn local_branches(&self) -> Result<Vec<String>, GitError> {
        let platform = self
            .inner
            .references()
            .map_err(|e| GitError::Open(e.to_string()))?;
        let iter = platform
            .prefixed("refs/heads/")
            .map_err(|e| GitError::Open(e.to_string()))?;
        let mut names = Vec::new();
        for reference in iter {
            let reference = reference.map_err(|e| GitError::Open(e.to_string()))?;
            names.push(reference.name().shorten().to_string());
        }
        Ok(names)
    }

    pub fn op_in_progress(&self) -> bool {
        let git_dir = self.inner.git_dir();
        ["MERGE_HEAD", "rebase-merge", "rebase-apply"]
            .iter()
            .any(|marker| git_dir.join(marker).exists())
    }
}
