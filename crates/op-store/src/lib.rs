use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::fs::OpenOptions;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use fs2::FileExt as _;
use op_task::{Frontmatter, Task, parse_id, rank, ref_id, ref_target, task_filename};

pub const STORE_DIR: &str = ".plan";

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
    #[error("task id already taken: {id}")]
    IdTaken { id: String },
    #[error("not a task id: {id:?}; a task id is a decimal number, like 42")]
    InvalidId { id: String },
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

    pub fn task_path(&self, id: &str) -> Result<PathBuf, StoreError> {
        self.task_file(id)?
            .ok_or_else(|| StoreError::NotFound { id: id.to_owned() })
    }

    fn task_file(&self, id: &str) -> Result<Option<PathBuf>, StoreError> {
        let number = parse_id(id).ok_or_else(|| StoreError::InvalidId { id: id.to_owned() })?;
        Ok(self.task_files()?.remove(&number))
    }

    // The file names carry a title slug the id does not (§3.1), so a task is found by the number its
    // name starts with rather than by a name built from its id. The lowest name wins, so a store
    // hand-edited into two files of one number resolves to the same one every time.
    fn task_files(&self) -> Result<BTreeMap<u64, PathBuf>, StoreError> {
        let mut files = BTreeMap::new();
        self.for_each_task_file(|path, number| match files.entry(number) {
            Entry::Vacant(slot) => {
                slot.insert(path);
            }
            Entry::Occupied(mut slot) if path < *slot.get() => {
                slot.insert(path);
            }
            Entry::Occupied(_) => {}
        })?;
        Ok(files)
    }

    fn for_each_task_file(&self, mut visit: impl FnMut(PathBuf, u64)) -> Result<(), StoreError> {
        let dir = self.tasks_dir();
        if !dir.exists() {
            return Ok(());
        }
        for entry in std::fs::read_dir(&dir)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            if let Some(number) = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .and_then(op_task::file_id)
            {
                visit(path, number);
            }
        }
        Ok(())
    }

    pub fn exists(&self, id: &str) -> bool {
        matches!(self.task_file(id), Ok(Some(_)))
    }

    pub fn task_ids(&self) -> Result<Vec<String>, StoreError> {
        Ok(self
            .task_files()?
            .into_keys()
            .map(|number| number.to_string())
            .collect())
    }

    // One scan and one open per task, for a caller that wants them all: resolving each id on its own
    // rescans the directory, which turns a whole-store read into quadratic work.
    pub fn read_all_raw(&self) -> Result<BTreeMap<String, String>, StoreError> {
        self.task_files()?
            .into_iter()
            .map(|(number, path)| {
                let id = number.to_string();
                let text = read_file(&path, &id)?;
                Ok((id, text))
            })
            .collect()
    }

    pub fn read(&self, id: &str) -> Result<Task, StoreError> {
        let path = self.task_path(id)?;
        Task::from_file_string(&read_file(&path, id)?).map_err(|err| match err {
            op_task::TaskError::MissingCreated => StoreError::MissingCreated {
                path,
                example: op_task::now().to_string(),
            },
            other => other.into(),
        })
    }

    pub fn read_raw(&self, id: &str) -> Result<String, StoreError> {
        read_file(&self.task_path(id)?, id)
    }

    // `number` is the task's identity and must come from the daemon's allocator, the single writer
    // that can see every local branch; the store itself has no repo-wide view to allocate from.
    pub fn create(&self, task: &Task, number: u64) -> Result<String, StoreError> {
        let title = single_title(&task.body)?;
        self.validate(None, None, &task.frontmatter)?;
        std::fs::create_dir_all(self.tasks_dir())?;
        // A file already carrying the number owns it whatever its slug says, so the check cannot be
        // left to the non-clobbering link below: a different title would name a different file.
        if self.task_file(&number.to_string())?.is_some() {
            return Err(StoreError::IdTaken {
                id: number.to_string(),
            });
        }
        let contents = self.in_file_form(task)?.to_file_string()?;
        let tmp = self.write_temp(contents.as_bytes())?;
        let result = self.link_id(&tmp, number, &title);
        // Clean up the temp link on every exit (success, a taken id, or an early error such as a
        // randomness failure) so no orphan .tmp is left behind.
        let _ = std::fs::remove_file(&tmp);
        result
    }

    // Publish the fully-written temp under its name with a non-clobbering hard link: the watcher only
    // ever sees a complete file, and a taken name is reported instead of overwriting another task.
    fn link_id(&self, tmp: &Path, number: u64, title: &str) -> Result<String, StoreError> {
        let path = self.tasks_dir().join(task_filename(number, title));
        let id = number.to_string();
        match std::fs::hard_link(tmp, &path) {
            Ok(()) => Ok(id),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Err(StoreError::IdTaken { id }),
            Err(e) => Err(e.into()),
        }
    }

    pub fn write(&self, id: &str, task: &Task) -> Result<(), StoreError> {
        self.task_path(id)?;
        self.with_lock(id, || {
            let old = self.read(id)?.frontmatter;
            self.validate(Some(id), Some(&old), &task.frontmatter)?;
            let contents = self.in_file_form(task)?.to_file_string()?;
            self.atomic_replace(id, contents.as_bytes())
        })
    }

    pub fn update(
        &self,
        id: &str,
        mutate: impl FnOnce(&mut Task) -> Result<(), StoreError>,
    ) -> Result<Task, StoreError> {
        self.task_path(id)?;
        self.with_lock(id, || {
            let mut task = self.read(id)?;
            let old = task.frontmatter.clone();
            mutate(&mut task)?;
            self.validate(Some(id), Some(&old), &task.frontmatter)?;
            let contents = self.in_file_form(&task)?.to_file_string()?;
            self.atomic_replace(id, contents.as_bytes())?;
            Ok(task)
        })
    }

    pub fn delete(&self, id: &str) -> Result<(), StoreError> {
        match std::fs::remove_file(self.task_path(id)?) {
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
        // Creation never conflicts (one file per task, one number per task), so a lock
        // only guards mutation of an existing task — open without creating, so a
        // lock never materializes a phantom empty task file.
        let path = self.task_path(id)?;
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
    // A task's own words for another task, in the form the file carries them (§3.1): the target's
    // file name, which only the store can spell. A reference whose task has since been deleted keeps
    // the bare number — it names no file, and inventing one would be a lie a reader could follow.
    fn in_file_form(&self, task: &Task) -> Result<Task, StoreError> {
        let files = self.task_files()?;
        let named =
            |reference: &String| match ref_id(reference).and_then(|number| files.get(&number)) {
                None => reference.clone(),
                Some(path) => {
                    let name =
                        op_task::task_ref(&path.file_name().unwrap_or_default().to_string_lossy());
                    match reference.split_once('#') {
                        Some((_, section)) => format!("{name}#{section}"),
                        None => name,
                    }
                }
            };
        let mut task = task.clone();
        task.frontmatter.parent = task.frontmatter.parent.as_ref().map(&named);
        task.frontmatter.dependencies = task.frontmatter.dependencies.iter().map(named).collect();
        Ok(task)
    }

    fn validate(
        &self,
        id: Option<&str>,
        old: Option<&Frontmatter>,
        new: &Frontmatter,
    ) -> Result<(), StoreError> {
        if let Some(parent) = &new.parent {
            let unchanged = old.and_then(|o| o.parent.as_deref()) == Some(parent.as_str());
            if !unchanged {
                let target = reject_dangling_ref(parent)?;
                if Some(target.as_str()) == id {
                    return Err(StoreError::Invalid(format!(
                        "task {parent} cannot be its own parent"
                    )));
                }
                if !self.exists(&target) {
                    return Err(StoreError::Invalid(format!(
                        "parent {parent} does not exist"
                    )));
                }
                if let Some(id) = id {
                    self.reject_parent_cycle(id, &target)?;
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
        for dependency in &new.dependencies {
            if old.is_some_and(|o| o.dependencies.contains(dependency)) {
                continue;
            }
            let target = reject_dangling_ref(dependency)?;
            if Some(target.as_str()) == id {
                return Err(StoreError::Invalid(format!(
                    "task cannot depend on itself: {dependency}"
                )));
            }
            if !self.exists(&target) {
                return Err(StoreError::Invalid(format!(
                    "dependency {dependency} does not exist"
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
        let path = self.task_path(id)?;
        let tmp = self.write_temp(bytes)?;
        std::fs::rename(&tmp, path)?;
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

// A task's title is its single `# H1` (§3.1). Reject bodies with zero, empty, or multiple level-1
// headings so a title carrying a newline (`"a\n# b"` → two H1s) or no text can never be persisted.
fn single_title(body: &str) -> Result<String, StoreError> {
    let mut h1s = op_md::headings(body).into_iter().filter(|h| h.level == 1);
    match (h1s.next(), h1s.next()) {
        (Some(h1), None) if !h1.text.trim().is_empty() => Ok(h1.text),
        _ => Err(StoreError::Invalid(
            "a task must have exactly one non-empty `# ` title heading".to_owned(),
        )),
    }
}

fn read_file(path: &Path, id: &str) -> Result<String, StoreError> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(text),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            Err(StoreError::NotFound { id: id.to_owned() })
        }
        Err(e) => Err(e.into()),
    }
}

fn reject_dangling_ref(reference: &str) -> Result<String, StoreError> {
    match ref_id(reference) {
        Some(number) => Ok(number.to_string()),
        None => Err(StoreError::InvalidId {
            id: ref_target(reference).to_owned(),
        }),
    }
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
