use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use gix::ObjectId;
use op_backend::{Backend as _, BackendError, Edit, Op, Origin, Snapshot as _};

use crate::objects::{self, GitSnapshot, storage};
use crate::{GitBackend, TASKS_NAME, TASKS_REF, lock};

pub const UNCOMMITTED_MESSAGE: &str = "Import task edits not yet committed";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Imported {
    pub revisions: usize,
    pub uncommitted: bool,
}

impl GitBackend {
    // Starts the tasks branch from `dir` of the checked-out branch: one revision for each commit
    // that changed it, with its author, time, and message, so every task keeps its history. The
    // edits in `checkout` that no commit holds yet follow as one more revision; a checkout without
    // `dir` has none.
    pub fn import(&self, checkout: &Path, dir: &str) -> Result<Imported, BackendError> {
        let inner = &self.inner;
        let tip = {
            let _writing = lock(&inner.writing);
            if inner.tip()?.is_some() {
                return Err(BackendError::Storage(format!(
                    "{TASKS_NAME} already exists, so there is nothing to import into"
                )));
            }
            let repo = inner.local();
            let tip = replay(&repo, dir)?;
            if let Some(tip) = tip {
                let moved =
                    objects::move_reference(&repo, TASKS_REF, None, tip, &inner.machine, "import")?;
                if !moved {
                    return Err(BackendError::Contended);
                }
                let changes =
                    objects::changes(&objects::Entries::new(), &inner.entries(Some(tip))?);
                inner.announce(Some(tip), changes, Origin::Local)?;
            }
            tip
        };
        let revisions = match tip {
            Some(tip) => count(&inner.local(), tip)?,
            None => 0,
        };
        let working_dir = checkout.join(dir);
        if !working_dir.is_dir() {
            return Ok(Imported {
                revisions,
                uncommitted: false,
            });
        }
        let working = scan(&working_dir)?;
        let head = GitSnapshot::of(&inner.repo, tip)?;
        let mut ops = Vec::new();
        for (path, bytes) in &working {
            if head.read(path)?.as_ref() != Some(bytes) {
                ops.push(Op::put(path.clone(), bytes.clone()));
            }
        }
        for path in head.files()? {
            if !working.contains_key(&path) {
                ops.push(Op::remove(path));
            }
        }
        let uncommitted = !ops.is_empty()
            && self
                .commit(&inner.machine, &mut |_| {
                    Ok(Edit::new(UNCOMMITTED_MESSAGE, ops.clone()))
                })?
                .is_some();
        Ok(Imported {
            revisions,
            uncommitted,
        })
    }
}

fn replay(repo: &gix::Repository, dir: &str) -> Result<Option<ObjectId>, BackendError> {
    let Ok(head) = repo.head_id() else {
        return Ok(None);
    };
    let mut commits: Vec<ObjectId> = repo
        .rev_walk([head.detach()])
        .first_parent_only()
        .all()
        .map_err(storage)?
        .map(|info| info.map(|info| info.id))
        .collect::<Result<_, _>>()
        .map_err(storage)?;
    commits.reverse();
    let mut tip = None;
    let mut last = None;
    for id in commits {
        let Some(tree) = objects::subtree(repo, objects::tree_of(repo, id)?, dir)? else {
            continue;
        };
        if Some(tree) == last {
            continue;
        }
        let commit = repo.find_commit(id).map_err(storage)?;
        let decoded = commit.decode().map_err(storage)?;
        let copy = gix::objs::Commit {
            tree,
            parents: tip.into_iter().collect(),
            author: decoded
                .author()
                .map_err(storage)?
                .to_owned()
                .map_err(storage)?,
            committer: decoded
                .committer()
                .map_err(storage)?
                .to_owned()
                .map_err(storage)?,
            encoding: None,
            message: decoded.message.to_owned(),
            extra_headers: Vec::new(),
        };
        tip = Some(repo.write_object(&copy).map_err(storage)?.detach());
        last = Some(tree);
    }
    Ok(tip)
}

fn count(repo: &gix::Repository, tip: ObjectId) -> Result<usize, BackendError> {
    Ok(repo.rev_walk([tip]).all().map_err(storage)?.count())
}

fn scan(root: &Path) -> io::Result<BTreeMap<String, Vec<u8>>> {
    let mut files = BTreeMap::new();
    if root.is_dir() {
        collect(root, "", &mut files)?;
    }
    Ok(files)
}

fn collect(dir: &Path, prefix: &str, files: &mut BTreeMap<String, Vec<u8>>) -> io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if name.starts_with('.') {
            continue;
        }
        let path = format!("{prefix}{name}");
        let kind = entry.file_type()?;
        if kind.is_dir() {
            collect(&entry.path(), &format!("{path}/"), files)?;
        } else if kind.is_file() {
            files.insert(path, std::fs::read(entry.path())?);
        }
    }
    Ok(())
}
