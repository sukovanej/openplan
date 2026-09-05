use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::fs::OpenOptions;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use fs2::FileExt as _;
use op_task::{
    Abbreviation, Frontmatter, Task, Timestamp, parse_id, rank, ref_id, ref_target, task_filename,
};

mod config;
mod tag;
pub use config::{CONFIG_FILE, Config, ConfigError};

pub const STORE_DIR: &str = ".plan";

const ID_ATTEMPTS: usize = 16;
const LOCK_ATTEMPTS: usize = 1024;
const HEX: &[u8; 16] = b"0123456789abcdef";

// One store, one abbreviation: a `Store` is always read under the abbreviation of the
// worktree the daemon serves, so a sibling worktree whose `config.toml` says something else — or has
// none — still renders one task as one key. The number names the file; the key is what the store's
// own refusals are phrased in, since every caller reached it through one.
#[derive(Debug, Clone)]
pub struct Store {
    root: PathBuf,
    abbreviation: Abbreviation,
}

#[derive(Debug, Clone)]
pub struct RawTask {
    pub text: String,
    // `None` where the filesystem cannot say — a platform without mtimes, or a file that vanished
    // between the scan and the stat.
    pub modified: Option<Timestamp>,
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("no {STORE_DIR}/ store found")]
    StoreMissing,
    #[error("no such task: {id}")]
    NotFound { id: String },
    #[error("task id already taken: {id}")]
    IdTaken { id: String },
    #[error("no such tag: {name}")]
    TagNotFound { name: String },
    #[error("tag already exists: {name}")]
    TagExists { name: String },
    #[error("tag {name} is used by {count} task(s) on this branch")]
    TagReferenced { name: String, count: usize },
    #[error("tag {name} is not registered on this branch")]
    TagUnregistered { name: String },
    #[error("not a task reference: {reference:?}; {}", op_task::REFERENCE_EXPECTED)]
    InvalidRef { reference: String },
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
    Config(#[from] ConfigError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Task(#[from] op_task::TaskError),
    #[error("{path}: {source}")]
    TagFile {
        path: PathBuf,
        #[source]
        source: op_task::tag::TagError,
    },
    #[error(transparent)]
    InvalidColor(#[from] op_task::tag::ParseColorError),
}

impl Store {
    pub fn open(root: impl AsRef<Path>, abbreviation: Abbreviation) -> Result<Self, StoreError> {
        let root = root.as_ref().to_path_buf();
        if root.join(STORE_DIR).is_dir() {
            Ok(Self { root, abbreviation })
        } else {
            Err(StoreError::StoreMissing)
        }
    }

    // The store at or above `start`, read under the abbreviation its own `config.toml` names — the
    // entry point every command and the daemon come in through, so a store with no abbreviation
    // stops them all before they can print an id it has no spelling for.
    pub fn discover(start: impl AsRef<Path>) -> Result<Self, StoreError> {
        let root = Self::discover_root(start)?;
        let abbreviation = Config::read(&root)?.abbreviation;
        Ok(Self { root, abbreviation })
    }

    fn discover_root(start: impl AsRef<Path>) -> Result<PathBuf, StoreError> {
        let start = start.as_ref();
        let base = if start.is_absolute() {
            start.to_path_buf()
        } else {
            std::env::current_dir()?.join(start)
        };
        base.ancestors()
            .find(|dir| dir.join(STORE_DIR).is_dir())
            .map(Path::to_path_buf)
            .ok_or(StoreError::StoreMissing)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn abbreviation(&self) -> Abbreviation {
        self.abbreviation
    }

    // The same store read under another abbreviation, for a caller that holds the store's current
    // one — a live config change — rather than the one this handle was opened with.
    pub fn with_abbreviation(&self, abbreviation: Abbreviation) -> Self {
        Self {
            root: self.root.clone(),
            abbreviation,
        }
    }

    fn key(&self, number: u64) -> String {
        self.abbreviation.format_key(number)
    }

    fn not_found(&self, number: u64) -> StoreError {
        StoreError::NotFound {
            id: self.key(number),
        }
    }

    pub fn plan_dir(&self) -> PathBuf {
        self.root.join(STORE_DIR)
    }

    pub fn tasks_dir(&self) -> PathBuf {
        self.plan_dir().join("tasks")
    }

    pub fn task_path(&self, id: u64) -> Result<PathBuf, StoreError> {
        self.task_file(id)?.ok_or_else(|| self.not_found(id))
    }

    fn task_file(&self, id: u64) -> Result<Option<PathBuf>, StoreError> {
        Ok(self.task_files()?.remove(&id))
    }

    // The file names carry a title slug the id does not, so a task is found by the number its
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

    pub fn exists(&self, id: u64) -> bool {
        matches!(self.task_file(id), Ok(Some(_)))
    }

    pub fn task_ids(&self) -> Result<Vec<u64>, StoreError> {
        Ok(self.task_files()?.into_keys().collect())
    }

    // One scan and one open per task, for a caller that wants them all: resolving each id on its own
    // rescans the directory, which turns a whole-store read into quadratic work.
    pub fn read_all_raw(&self) -> Result<BTreeMap<u64, RawTask>, StoreError> {
        self.task_files()?
            .into_iter()
            .map(|(number, path)| {
                let text = self.read_file(&path, number)?;
                Ok((
                    number,
                    RawTask {
                        text,
                        modified: modified_at(&path),
                    },
                ))
            })
            .collect()
    }

    pub fn read(&self, id: u64) -> Result<Task, StoreError> {
        let path = self.task_path(id)?;
        Task::from_file_string(&self.read_file(&path, id)?).map_err(|err| match err {
            op_task::TaskError::MissingCreated => StoreError::MissingCreated {
                path,
                example: op_task::now().to_string(),
            },
            other => other.into(),
        })
    }

    pub fn read_raw(&self, id: u64) -> Result<String, StoreError> {
        self.read_file(&self.task_path(id)?, id)
    }

    fn read_file(&self, path: &Path, id: u64) -> Result<String, StoreError> {
        match std::fs::read_to_string(path) {
            Ok(text) => Ok(text),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Err(self.not_found(id)),
            Err(e) => Err(e.into()),
        }
    }

    // `number` is the task's identity and must come from the daemon's allocator, the single writer
    // that can see every local branch; the store itself has no repo-wide view to allocate from.
    pub fn create(&self, task: &Task, number: u64) -> Result<u64, StoreError> {
        let title = single_title(&task.body)?;
        // Before the validation, because the tags the new task carries are validated against a
        // registry this call is what creates.
        self.seed_default_tags()?;
        self.validate(None, None, &task.frontmatter)?;
        std::fs::create_dir_all(self.tasks_dir())?;
        // A file already carrying the number owns it whatever its slug says, so the check cannot be
        // left to the non-clobbering link below: a different title would name a different file.
        if self.task_file(number)?.is_some() {
            return Err(StoreError::IdTaken {
                id: self.key(number),
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
    fn link_id(&self, tmp: &Path, number: u64, title: &str) -> Result<u64, StoreError> {
        let path = self.tasks_dir().join(task_filename(number, title));
        match std::fs::hard_link(tmp, &path) {
            Ok(()) => Ok(number),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Err(StoreError::IdTaken {
                id: self.key(number),
            }),
            Err(e) => Err(e.into()),
        }
    }

    pub fn write(&self, id: u64, task: &Task) -> Result<(), StoreError> {
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
        id: u64,
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

    // A verbatim overwrite of a task's file, locked and atomic like every other write but bypassing
    // the model: `lint --fix` splices bytes it computed itself and must not have them reflowed
    // through `Task::to_file_string`. Allocates no id and resolves no branch, so it is an
    // out-of-band writer, not a daemon write.
    pub fn replace_raw(&self, id: u64, bytes: &[u8]) -> Result<(), StoreError> {
        self.task_path(id)?;
        self.with_lock(id, || self.atomic_replace(id, bytes))
    }

    pub fn delete(&self, id: u64) -> Result<(), StoreError> {
        match std::fs::remove_file(self.task_path(id)?) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Err(self.not_found(id)),
            Err(e) => Err(e.into()),
        }
    }

    pub fn with_lock<T>(
        &self,
        id: u64,
        f: impl FnOnce() -> Result<T, StoreError>,
    ) -> Result<T, StoreError> {
        let path = self.task_path(id)?;
        with_file_lock(&path, || self.not_found(id), f)
    }

    // Only references this write newly introduces are validated. A parent/dep that was
    // already persisted (and may now dangle because its target was deleted) must not block
    // an unrelated edit like a status change — otherwise deleting one task bricks another.
    // A task's own words for another task, in the form the file carries them: the target's
    // file name, which only the store can spell — in the frontmatter and in the body's `[[…]]` alike.
    // A reference whose task has since been deleted keeps the bare number — it names no file, and
    // inventing one would be a lie a reader could follow.
    fn in_file_form(&self, task: &Task) -> Result<Task, StoreError> {
        let files = self.task_files()?;
        let named = |reference: &str| match ref_id(reference).and_then(|number| files.get(&number))
        {
            None => reference.to_owned(),
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
        task.frontmatter.parent = task.frontmatter.parent.as_deref().map(named);
        task.frontmatter.dependencies = task
            .frontmatter
            .dependencies
            .iter()
            .map(|reference| named(reference))
            .collect();
        // A body's reference is read by a markdown renderer, which knows no bare number: one
        // that names no file is written as the key, so it still resolves once that task exists.
        task.body = body_in_file_form(&task.body, |reference| {
            let named = named(reference);
            if named != reference {
                return named;
            }
            match parse_id(ref_target(reference)) {
                Some(number) => with_section(&self.key(number), reference),
                None => named,
            }
        });
        Ok(task)
    }

    fn validate(
        &self,
        id: Option<u64>,
        old: Option<&Frontmatter>,
        new: &Frontmatter,
    ) -> Result<(), StoreError> {
        if let Some(parent) = &new.parent {
            let unchanged = old.and_then(|o| o.parent.as_deref()) == Some(parent.as_str());
            if !unchanged {
                let target = reject_dangling_ref(parent)?;
                if Some(target) == id {
                    return Err(StoreError::Invalid(format!(
                        "task {} cannot be its own parent",
                        self.key(target)
                    )));
                }
                if !self.exists(target) {
                    return Err(StoreError::Invalid(format!(
                        "parent {} does not exist",
                        self.key(target)
                    )));
                }
                if let Some(id) = id {
                    self.reject_parent_cycle(id, target)?;
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
            if Some(target) == id {
                return Err(StoreError::Invalid(format!(
                    "task cannot depend on itself: {}",
                    self.key(target)
                )));
            }
            if !self.exists(target) {
                return Err(StoreError::Invalid(format!(
                    "dependency {} does not exist",
                    self.key(target)
                )));
            }
        }
        // The whole set, not just what this write adds: a task holding a tag this branch does not
        // register must drop it in the same write, so no edit carries a dangling name forward.
        if !new.tags.is_empty() {
            let registered = self.tag_names()?;
            for tag in &new.tags {
                if !registered.contains(tag) {
                    return Err(StoreError::TagUnregistered { name: tag.clone() });
                }
            }
        }
        Ok(())
    }

    // Reparenting `id` under `parent` must not make `id` an ancestor of itself. Walk up from
    // `parent`: reaching `id` would close a cycle, so refuse. Bounded by a visited set so a
    // pre-existing cycle among other tasks is reported rather than looped on forever.
    fn reject_parent_cycle(&self, id: u64, parent: u64) -> Result<(), StoreError> {
        let mut cursor = Some(parent);
        let mut seen = std::collections::HashSet::new();
        while let Some(current) = cursor {
            if current == id {
                return Err(StoreError::Invalid(format!(
                    "cannot reparent {} under its own descendant {}",
                    self.key(id),
                    self.key(parent)
                )));
            }
            if !seen.insert(current) {
                break;
            }
            cursor = match self.read(current) {
                Ok(task) => task.frontmatter.parent.as_deref().and_then(ref_id),
                Err(StoreError::NotFound { .. }) => None,
                Err(err) => return Err(err),
            };
        }
        Ok(())
    }

    fn atomic_replace(&self, id: u64, bytes: &[u8]) -> Result<(), StoreError> {
        atomic_replace(&self.task_path(id)?, bytes)
    }

    fn write_temp(&self, bytes: &[u8]) -> Result<PathBuf, StoreError> {
        write_temp(&self.tasks_dir(), bytes)
    }
}

pub(crate) fn with_file_lock<T>(
    path: &Path,
    missing: impl Fn() -> StoreError,
    f: impl FnOnce() -> Result<T, StoreError>,
) -> Result<T, StoreError> {
    // Creation never conflicts (one file per name), so a lock only guards mutation of an existing
    // file — open without creating, so a lock never materializes a phantom empty one.
    for _ in 0..LOCK_ATTEMPTS {
        let file = match OpenOptions::new().read(true).open(path) {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Err(missing()),
            Err(e) => return Err(e.into()),
        };
        file.lock_exclusive()?;
        // A concurrent atomic replace renames a fresh inode over the path while we wait
        // for the lock, leaving our lock guarding a stale, unlinked inode. Re-open until
        // the inode we locked is the one currently at the path, so writes truly serialize.
        if same_inode(&file, path)? {
            let result = f();
            let _ = fs2::FileExt::unlock(&file);
            return result;
        }
        let _ = fs2::FileExt::unlock(&file);
    }
    Err(StoreError::Io(io::Error::new(
        io::ErrorKind::WouldBlock,
        format!("lock contention on {} did not settle", path.display()),
    )))
}

// Atomic replace of an existing file: the rename swaps the fully-written temp into place in one
// step, so a watcher never observes a torn file. Callers hold the per-file lock.
pub(crate) fn atomic_replace(path: &Path, bytes: &[u8]) -> Result<(), StoreError> {
    let dir = path.parent().ok_or_else(|| {
        StoreError::Io(io::Error::other(format!(
            "{} names no directory to write into",
            path.display()
        )))
    })?;
    let tmp = write_temp(dir, bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

pub(crate) fn write_temp(dir: &Path, bytes: &[u8]) -> Result<PathBuf, StoreError> {
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

// A task's title is its single `# H1`. Reject bodies with zero, empty, or multiple level-1
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

fn with_section(target: &str, reference: &str) -> String {
    match reference.split_once('#') {
        Some((_, section)) => format!("{target}#{section}"),
        None => target.to_owned(),
    }
}

fn reject_dangling_ref(reference: &str) -> Result<u64, StoreError> {
    ref_id(reference).ok_or_else(|| StoreError::InvalidRef {
        reference: ref_target(reference).to_owned(),
    })
}

// A body's `[[…]]` put in file form by the same naming as the frontmatter's refs. Text that names no
// task at all is left exactly as written — ordinary bracketed prose is not a reference.
fn body_in_file_form(body: &str, named: impl Fn(&str) -> String) -> String {
    let mut out = String::new();
    let mut last = 0;
    for (span, inner) in op_task::body_ref_spans(body) {
        let renamed = named(inner);
        if renamed == inner {
            continue;
        }
        out.push_str(&body[last..span.start]);
        out.push_str("[[");
        out.push_str(&renamed);
        out.push_str("]]");
        last = span.end;
    }
    if last == 0 {
        return body.to_owned();
    }
    out.push_str(&body[last..]);
    out
}

// Whole seconds, the resolution every other time this reports carries — a commit's author date and
// a task's `created` — so one task's dates cannot be compared at two different precisions.
fn modified_at(path: &Path) -> Option<Timestamp> {
    let at = Timestamp::try_from(std::fs::metadata(path).ok()?.modified().ok()?).ok()?;
    Timestamp::from_second(at.as_second()).ok()
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
