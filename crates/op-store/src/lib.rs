use std::fs::OpenOptions;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use fs2::FileExt as _;
use op_task::{Frontmatter, Task, rank};

pub const STORE_DIR: &str = ".plan";

const SLUG_MAX: usize = 32;
const ID_ATTEMPTS: usize = 16;
const LOCK_ATTEMPTS: usize = 1024;
const HEX: &[u8; 16] = b"0123456789abcdef";

#[derive(Debug, Clone)]
pub struct Store {
    root: PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("no {STORE_DIR}/ store found")]
    StoreMissing,
    #[error("no such task: {id}")]
    NotFound { id: String },
    #[error("{0}")]
    Invalid(String),
    #[error(
        "{path} has no `created:` field, so it cannot be written — a write must not invent when \
         the task was created.\n\nAdd the field to its frontmatter by hand:\n\n    created: \
         {example}\n\nIf the file is already committed, the date it first appeared is:\n\n    git \
         log --diff-filter=A --format=%aI -1 -- {path}"
    )]
    MissingCreated { path: PathBuf, example: String },
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Task(#[from] op_task::TaskError),
}

impl Store {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, StoreError> {
        let root = root.as_ref().to_path_buf();
        if root.join(STORE_DIR).is_dir() {
            Ok(Self { root })
        } else {
            Err(StoreError::StoreMissing)
        }
    }

    pub fn discover(start: impl AsRef<Path>) -> Result<Self, StoreError> {
        let start = start.as_ref();
        let base = if start.is_absolute() {
            start.to_path_buf()
        } else {
            std::env::current_dir()?.join(start)
        };
        for dir in base.ancestors() {
            if dir.join(STORE_DIR).is_dir() {
                return Ok(Self {
                    root: dir.to_path_buf(),
                });
            }
        }
        Err(StoreError::StoreMissing)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn plan_dir(&self) -> PathBuf {
        self.root.join(STORE_DIR)
    }

    pub fn tasks_dir(&self) -> PathBuf {
        self.plan_dir().join("tasks")
    }

    pub fn task_path(&self, id: &str) -> PathBuf {
        self.tasks_dir().join(format!("{id}.md"))
    }

    pub fn exists(&self, id: &str) -> bool {
        self.task_path(id).is_file()
    }

    pub fn task_ids(&self) -> Result<Vec<String>, StoreError> {
        let dir = self.tasks_dir();
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut ids = Vec::new();
        for entry in std::fs::read_dir(&dir)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                ids.push(stem.to_owned());
            }
        }
        ids.sort();
        Ok(ids)
    }

    pub fn read(&self, id: &str) -> Result<Task, StoreError> {
        Task::from_file_string(&self.read_raw(id)?).map_err(|err| match err {
            op_task::TaskError::MissingCreated => StoreError::MissingCreated {
                path: self.task_path(id),
                example: op_task::now().to_string(),
            },
            other => other.into(),
        })
    }

    pub fn read_raw(&self, id: &str) -> Result<String, StoreError> {
        match std::fs::read_to_string(self.task_path(id)) {
            Ok(text) => Ok(text),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                Err(StoreError::NotFound { id: id.to_owned() })
            }
            Err(e) => Err(e.into()),
        }
    }

    pub fn create(&self, task: &Task) -> Result<String, StoreError> {
        let title = single_title(&task.body)?;
        self.validate(None, None, &task.frontmatter)?;
        std::fs::create_dir_all(self.tasks_dir())?;
        let contents = task.to_file_string()?;
        let tmp = self.write_temp(contents.as_bytes())?;
        let result = self.link_new_id(&tmp, &slug(&title));
        // Clean up the temp link on every exit (success, collision-exhaustion, or an
        // early error such as a randomness failure) so no orphan .tmp is left behind.
        let _ = std::fs::remove_file(&tmp);
        result
    }

    // Publish the fully-written temp under a fresh id with a non-clobbering hard link:
    // the watcher only ever sees a complete file, and a slug collision links onto an
    // existing name (AlreadyExists) instead of overwriting another task.
    fn link_new_id(&self, tmp: &Path, base: &str) -> Result<String, StoreError> {
        for _ in 0..ID_ATTEMPTS {
            let id = format!("{base}-{}", rand_hex(2)?);
            match std::fs::hard_link(tmp, self.task_path(&id)) {
                Ok(()) => return Ok(id),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.into()),
            }
        }
        Err(StoreError::Io(io::Error::other(format!(
            "could not allocate a unique id for {base:?} after {ID_ATTEMPTS} attempts"
        ))))
    }

    pub fn write(&self, id: &str, task: &Task) -> Result<(), StoreError> {
        if !self.exists(id) {
            return Err(StoreError::NotFound { id: id.to_owned() });
        }
        let contents = task.to_file_string()?;
        self.with_lock(id, || {
            let old = self.read(id)?.frontmatter;
            self.validate(Some(id), Some(&old), &task.frontmatter)?;
            self.atomic_replace(id, contents.as_bytes())
        })
    }

    pub fn update(
        &self,
        id: &str,
        mutate: impl FnOnce(&mut Task) -> Result<(), StoreError>,
    ) -> Result<Task, StoreError> {
        if !self.exists(id) {
            return Err(StoreError::NotFound { id: id.to_owned() });
        }
        self.with_lock(id, || {
            let mut task = self.read(id)?;
            let old = task.frontmatter.clone();
            mutate(&mut task)?;
            self.validate(Some(id), Some(&old), &task.frontmatter)?;
            let contents = task.to_file_string()?;
            self.atomic_replace(id, contents.as_bytes())?;
            Ok(task)
        })
    }

    pub fn delete(&self, id: &str) -> Result<(), StoreError> {
        match std::fs::remove_file(self.task_path(id)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                Err(StoreError::NotFound { id: id.to_owned() })
            }
            Err(e) => Err(e.into()),
        }
    }

    pub fn with_lock<T>(
        &self,
        id: &str,
        f: impl FnOnce() -> Result<T, StoreError>,
    ) -> Result<T, StoreError> {
        // Creation never conflicts (one file per task, random-slug ids), so a lock
        // only guards mutation of an existing task — open without creating, so a
        // lock never materializes a phantom empty task file.
        let path = self.task_path(id);
        for _ in 0..LOCK_ATTEMPTS {
            let file = match OpenOptions::new().read(true).open(&path) {
                Ok(file) => file,
                Err(e) if e.kind() == io::ErrorKind::NotFound => {
                    return Err(StoreError::NotFound { id: id.to_owned() });
                }
                Err(e) => return Err(e.into()),
            };
            file.lock_exclusive()?;
            // A concurrent atomic replace renames a fresh inode over the path while we wait
            // for the lock, leaving our lock guarding a stale, unlinked inode. Re-open until
            // the inode we locked is the one currently at the path, so writes truly serialize.
            if same_inode(&file, &path)? {
                let result = f();
                let _ = fs2::FileExt::unlock(&file);
                return result;
            }
            let _ = fs2::FileExt::unlock(&file);
        }
        Err(StoreError::Io(io::Error::new(
            io::ErrorKind::WouldBlock,
            format!("lock contention on {id} did not settle"),
        )))
    }

    // Only references this write newly introduces are validated. A parent/dep that was
    // already persisted (and may now dangle because its target was deleted) must not block
    // an unrelated edit like a status change — otherwise deleting one task bricks another.
    fn validate(
        &self,
        id: Option<&str>,
        old: Option<&Frontmatter>,
        new: &Frontmatter,
    ) -> Result<(), StoreError> {
        if let Some(parent) = &new.parent {
            let unchanged = old.and_then(|o| o.parent.as_deref()) == Some(parent.as_str());
            if !unchanged {
                if Some(parent.as_str()) == id {
                    return Err(StoreError::Invalid(format!(
                        "task {parent} cannot be its own parent"
                    )));
                }
                if !self.exists(parent) {
                    return Err(StoreError::Invalid(format!(
                        "parent {parent} does not exist"
                    )));
                }
                if let Some(id) = id {
                    self.reject_parent_cycle(id, parent)?;
                }
            }
        }
        if let Some(value) = &new.rank {
            let unchanged = old.and_then(|o| o.rank.as_deref()) == Some(value.as_str());
            if !unchanged && !rank::is_valid(value) {
                return Err(StoreError::Invalid(format!(
                    "rank {value} is not a base-36 key (0-9a-z)"
                )));
            }
        }
        for dep in &new.deps {
            if old.is_some_and(|o| o.deps.contains(dep)) {
                continue;
            }
            let target = dep.split('#').next().unwrap_or(dep);
            if Some(target) == id {
                return Err(StoreError::Invalid(format!(
                    "task cannot depend on itself: {dep}"
                )));
            }
            if !self.exists(target) {
                return Err(StoreError::Invalid(format!(
                    "dependency {dep} does not exist"
                )));
            }
        }
        Ok(())
    }

    // Reparenting `id` under `parent` must not make `id` an ancestor of itself. Walk up from
    // `parent`: reaching `id` would close a cycle, so refuse. Bounded by a visited set so a
    // pre-existing cycle among other tasks is reported rather than looped on forever.
    fn reject_parent_cycle(&self, id: &str, parent: &str) -> Result<(), StoreError> {
        let mut cursor = Some(parent.to_owned());
        let mut seen = std::collections::HashSet::new();
        while let Some(current) = cursor {
            if current == id {
                return Err(StoreError::Invalid(format!(
                    "cannot reparent {id} under its own descendant {parent}"
                )));
            }
            if !seen.insert(current.clone()) {
                break;
            }
            cursor = match self.read(&current) {
                Ok(task) => task.frontmatter.parent,
                Err(StoreError::NotFound { .. }) => None,
                Err(err) => return Err(err),
            };
        }
        Ok(())
    }

    // Atomic replace of an existing task: the rename swaps the fully-written temp into place
    // in one step, so a watcher never observes a torn file. Callers hold the per-file lock.
    fn atomic_replace(&self, id: &str, bytes: &[u8]) -> Result<(), StoreError> {
        let tmp = self.write_temp(bytes)?;
        std::fs::rename(&tmp, self.task_path(id))?;
        Ok(())
    }

    fn write_temp(&self, bytes: &[u8]) -> Result<PathBuf, StoreError> {
        let dir = self.tasks_dir();
        for _ in 0..ID_ATTEMPTS {
            let path = dir.join(format!(".op-{}-{}.tmp", std::process::id(), rand_hex(4)?));
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(mut file) => {
                    file.write_all(bytes)?;
                    return Ok(path);
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.into()),
            }
        }
        Err(StoreError::Io(io::Error::other(format!(
            "could not open a temp file after {ID_ATTEMPTS} attempts"
        ))))
    }
}

// A task's identity is its single `# H1` title (§3.1). Reject bodies with zero, empty,
// or multiple level-1 headings so a title carrying a newline (`"a\n# b"` → two H1s) or no
// text can never be persisted.
fn single_title(body: &str) -> Result<String, StoreError> {
    let mut h1s = op_md::headings(body).into_iter().filter(|h| h.level == 1);
    match (h1s.next(), h1s.next()) {
        (Some(h1), None) if !h1.text.trim().is_empty() => Ok(h1.text),
        _ => Err(StoreError::Invalid(
            "a task must have exactly one non-empty `# ` title heading".to_owned(),
        )),
    }
}

fn slug(title: &str) -> String {
    let mut out = String::new();
    let mut pending_dash = false;
    for ch in title.chars() {
        if ch.is_ascii_alphanumeric() {
            if pending_dash && !out.is_empty() {
                out.push('-');
            }
            pending_dash = false;
            out.push(ch.to_ascii_lowercase());
            if out.len() >= SLUG_MAX {
                break;
            }
        } else {
            pending_dash = true;
        }
    }
    if out.is_empty() {
        out.push_str("task");
    }
    out
}

fn same_inode(file: &std::fs::File, path: &Path) -> Result<bool, StoreError> {
    use std::os::unix::fs::MetadataExt as _;
    let locked = file.metadata()?;
    match std::fs::metadata(path) {
        Ok(current) => Ok(locked.ino() == current.ino() && locked.dev() == current.dev()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}

fn rand_hex(n_bytes: usize) -> Result<String, StoreError> {
    let mut buf = vec![0u8; n_bytes];
    getrandom::fill(&mut buf).map_err(|e| StoreError::Io(io::Error::other(e.to_string())))?;
    let mut out = String::with_capacity(n_bytes * 2);
    for b in buf {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    Ok(out)
}
