use std::collections::{BTreeMap, BTreeSet};

use crate::{BackendError, Change, ChangeKind, Op, RevisionId};

pub trait Snapshot: Send + Sync {
    fn revision(&self) -> Option<&RevisionId>;

    fn read(&self, path: &str) -> Result<Option<Vec<u8>>, BackendError>;

    fn files(&self) -> Result<Vec<String>, BackendError>;

    fn list(&self, dir: &str) -> Result<Vec<String>, BackendError> {
        let prefix = format!("{}/", dir.trim_end_matches('/'));
        Ok(self
            .files()?
            .into_iter()
            .filter(|path| {
                path.strip_prefix(&prefix)
                    .is_some_and(|name| !name.contains('/'))
            })
            .collect())
    }

    fn read_text(&self, path: &str) -> Result<Option<String>, BackendError> {
        self.read(path)?
            .map(|bytes| {
                String::from_utf8(bytes).map_err(|_| BackendError::NotText(path.to_owned()))
            })
            .transpose()
    }
}

pub fn diff(from: &dyn Snapshot, to: &dyn Snapshot) -> Result<Vec<Change>, BackendError> {
    let paths: BTreeSet<String> = from.files()?.into_iter().chain(to.files()?).collect();
    let mut changes = Vec::new();
    for path in paths {
        let kind = match (from.read(&path)?, to.read(&path)?) {
            (None, Some(_)) => ChangeKind::Added,
            (Some(_), None) => ChangeKind::Removed,
            (Some(old), Some(new)) if old != new => ChangeKind::Modified,
            _ => continue,
        };
        changes.push(Change::new(path, kind));
    }
    Ok(changes)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemorySnapshot {
    revision: Option<RevisionId>,
    files: BTreeMap<String, Vec<u8>>,
}

impl MemorySnapshot {
    pub fn new(revision: Option<RevisionId>, files: BTreeMap<String, Vec<u8>>) -> Self {
        Self { revision, files }
    }

    pub fn copy_of(snapshot: &dyn Snapshot) -> Result<Self, BackendError> {
        let mut files = BTreeMap::new();
        for path in snapshot.files()? {
            if let Some(bytes) = snapshot.read(&path)? {
                files.insert(path, bytes);
            }
        }
        Ok(Self::new(snapshot.revision().cloned(), files))
    }

    pub fn into_files(self) -> BTreeMap<String, Vec<u8>> {
        self.files
    }
}

impl Snapshot for MemorySnapshot {
    fn revision(&self) -> Option<&RevisionId> {
        self.revision.as_ref()
    }

    fn read(&self, path: &str) -> Result<Option<Vec<u8>>, BackendError> {
        Ok(self.files.get(path).cloned())
    }

    fn files(&self) -> Result<Vec<String>, BackendError> {
        Ok(self.files.keys().cloned().collect())
    }
}

pub struct Overlay<'a> {
    base: &'a dyn Snapshot,
    edits: BTreeMap<String, Option<Vec<u8>>>,
}

impl<'a> Overlay<'a> {
    pub fn new(base: &'a dyn Snapshot) -> Self {
        Self {
            base,
            edits: BTreeMap::new(),
        }
    }

    pub fn set(&mut self, path: impl Into<String>, content: Option<Vec<u8>>) {
        self.edits.insert(path.into(), content);
    }

    pub fn apply(&mut self, ops: impl IntoIterator<Item = Op>) {
        for op in ops {
            match op {
                Op::Put { path, bytes } => self.set(path, Some(bytes)),
                Op::Remove { path } => self.set(path, None),
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.edits.is_empty()
    }

    pub fn into_ops(self) -> Vec<Op> {
        self.edits
            .into_iter()
            .map(|(path, content)| match content {
                Some(bytes) => Op::Put { path, bytes },
                None => Op::Remove { path },
            })
            .collect()
    }
}

impl Snapshot for Overlay<'_> {
    fn revision(&self) -> Option<&RevisionId> {
        None
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
