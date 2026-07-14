use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Repo {
    inner: gix::ThreadSafeRepository,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Worktree {
    pub path: PathBuf,
    pub branch: Option<String>,
    pub op_in_progress: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("could not open git repository: {0}")]
    Open(String),
    #[error("no such branch: {0}")]
    NoSuchBranch(String),
    #[error("object database error: {0}")]
    Object(String),
}

impl Repo {
    pub fn discover(path: impl AsRef<Path>) -> Result<Self, GitError> {
        let inner = gix::ThreadSafeRepository::discover(path.as_ref())
            .map_err(|e| GitError::Open(e.to_string()))?;
        Ok(Self { inner })
    }

    fn repo(&self) -> gix::Repository {
        self.inner.to_thread_local()
    }

    pub fn local_branches(&self) -> Result<Vec<String>, GitError> {
        let repo = self.repo();
        let platform = repo
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
        op_markers_present(self.repo().git_dir())
    }

    // Every worktree (main + linked) with a working copy on disk, paired with its checked-out
    // branch. A branch appears at most once — git allows it checked out in one worktree only.
    pub fn worktrees(&self) -> Result<Vec<Worktree>, GitError> {
        let repo = self.repo();
        let mut out = Vec::new();
        if let Ok(main) = repo.main_repo() {
            out.extend(worktree_of(&main));
        }
        for proxy in repo
            .worktrees()
            .map_err(|e| GitError::Open(e.to_string()))?
        {
            if let Ok(linked) = proxy.into_repo_with_possibly_inaccessible_worktree() {
                out.extend(worktree_of(&linked));
            }
        }
        Ok(out)
    }

    pub fn branch_task_blobs(&self, branch: &str) -> Result<Vec<(String, String)>, GitError> {
        let repo = self.repo();
        let mut reference = repo
            .try_find_reference(format!("refs/heads/{branch}").as_str())
            .map_err(|e| GitError::Open(e.to_string()))?
            .ok_or_else(|| GitError::NoSuchBranch(branch.to_owned()))?;
        let tree = reference
            .peel_to_tree()
            .map_err(|e| GitError::Object(e.to_string()))?;
        let Some(entry) = tree
            .lookup_entry_by_path(".plan/tasks")
            .map_err(|e| GitError::Object(e.to_string()))?
        else {
            return Ok(Vec::new());
        };
        if !entry.mode().is_tree() {
            return Ok(Vec::new());
        }
        let subtree = entry
            .object()
            .map_err(|e| GitError::Object(e.to_string()))?
            .into_tree();
        let mut out = Vec::new();
        for entry in subtree.iter() {
            let entry = entry.map_err(|e| GitError::Object(e.to_string()))?;
            if !entry.mode().is_blob() {
                continue;
            }
            if let Some(id) = entry.filename().to_string().strip_suffix(".md") {
                out.push((id.to_owned(), entry.oid().to_string()));
            }
        }
        Ok(out)
    }

    pub fn read_blob(&self, oid_hex: &str) -> Result<Vec<u8>, GitError> {
        let oid = gix::ObjectId::from_hex(oid_hex.as_bytes())
            .map_err(|e| GitError::Object(e.to_string()))?;
        let repo = self.repo();
        let object = repo
            .find_object(oid)
            .map_err(|e| GitError::Object(e.to_string()))?;
        Ok(object.detach().data)
    }

    pub fn hash_blob(&self, bytes: &[u8]) -> Result<String, GitError> {
        let oid = gix::objs::compute_hash(self.repo().object_hash(), gix::objs::Kind::Blob, bytes)
            .map_err(|e| GitError::Object(e.to_string()))?;
        Ok(oid.to_string())
    }
}

fn worktree_of(repo: &gix::Repository) -> Option<Worktree> {
    let path = repo.workdir()?.to_path_buf();
    if !path.is_dir() {
        return None;
    }
    let branch = repo
        .head_name()
        .ok()
        .flatten()
        .map(|name| name.shorten().to_string());
    Some(Worktree {
        path,
        branch,
        op_in_progress: op_markers_present(repo.git_dir()),
    })
}

fn op_markers_present(git_dir: &Path) -> bool {
    ["MERGE_HEAD", "rebase-merge", "rebase-apply"]
        .iter()
        .any(|marker| git_dir.join(marker).exists())
}
