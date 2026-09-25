use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use op_backend::{
    Actor, Backend, BackendError, BackendEvent, ChangeKind, Committed, Events, HeadMoved, LogEntry,
    LogQuery, MemorySnapshot, Op, Origin, Remote, RevisionId, Snapshot, Write, check_path,
};
use tokio::sync::broadcast;

mod disk;
mod history;
mod watch;

use history::{History, parse_id};

pub const HISTORY_FILE: &str = ".history.sqlite";
pub const EXTERNAL_MESSAGE: &str = "Edit outside openplan";

const WRITE_ATTEMPTS: usize = 8;

#[derive(Debug, Clone)]
pub struct Options {
    pub watch: bool,
    pub external_author: Actor,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            watch: false,
            external_author: Actor::new("filesystem"),
        }
    }
}

pub struct LocalBackend {
    inner: Arc<Inner>,
    _watcher: Option<watch::Watcher>,
}

struct Inner {
    root: PathBuf,
    external_author: Actor,
    state: Mutex<State>,
    events: Events,
}

struct State {
    history: History,
    head: Arc<MemorySnapshot>,
}

enum Source {
    Openplan,
    Disk,
}

impl LocalBackend {
    pub fn open(root: impl AsRef<Path>, options: Options) -> Result<Self, BackendError> {
        let root = root.as_ref().to_path_buf();
        std::fs::create_dir_all(&root)?;
        let history = History::open(&root.join(HISTORY_FILE))?;
        let head = Arc::new(history.snapshot(history.latest()?)?);
        let inner = Arc::new(Inner {
            root,
            external_author: options.external_author,
            state: Mutex::new(State { history, head }),
            events: Events::default(),
        });
        inner.finish_interrupted()?;
        inner.refresh()?;
        let watcher = match options.watch {
            true => inner.start_watch(),
            false => None,
        };
        Ok(Self {
            inner,
            _watcher: watcher,
        })
    }

    pub fn root(&self) -> &Path {
        &self.inner.root
    }
}

impl Inner {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().expect("local backend mutex poisoned")
    }

    fn start_watch(self: &Arc<Self>) -> Option<watch::Watcher> {
        let watched = Arc::downgrade(self);
        let started = watch::Watcher::start(&self.root, move || {
            if let Some(inner) = watched.upgrade()
                && let Err(err) = inner.refresh()
            {
                tracing::warn!(root = %inner.root.display(), error = %err, "hand edits not recorded");
            }
        });
        match started {
            Ok(watcher) => Some(watcher),
            Err(err) => {
                tracing::warn!(root = %self.root.display(), error = %err, "hand edits are picked up only on refresh");
                None
            }
        }
    }

    fn finish_interrupted(&self) -> Result<(), BackendError> {
        let state = self.lock();
        for id in state.history.unsettled()? {
            for path in state.history.changed_paths(id)? {
                disk::write(&self.root, &path, state.head.read(&path)?.as_deref())?;
            }
            state.history.settle(id)?;
        }
        Ok(())
    }

    // Another process can write the same history, so the cached head is only a guess until this
    // confirms it. A move it finds is announced, so listeners hear every move in order.
    fn catch_up(&self, state: &mut State) -> Result<Option<HeadMoved>, BackendError> {
        let latest = state.history.latest()?;
        if latest.map(history::revision_id).as_ref() == state.head.revision() {
            return Ok(None);
        }
        let next = Arc::new(state.history.snapshot(latest)?);
        let changes = op_backend::diff(&*state.head, &*next)?;
        let from = state.head.revision().cloned();
        state.head = next;
        let Some(latest) = latest else {
            return Ok(None);
        };
        let revision = state.history.revision(latest)?;
        let committed = Committed { revision, changes };
        Ok(Some(self.announce(from, &committed, Origin::External)))
    }

    // Hand edits the watcher has not settled yet are recorded first, so the write sees them and
    // never overwrites one.
    fn commit(&self, author: &Actor, write: Write<'_>) -> Result<Option<Committed>, BackendError> {
        let mut state = self.lock();
        for _ in 0..WRITE_ATTEMPTS {
            self.absorb(&mut state)?;
            let from = state.head.revision().cloned();
            let edit = write(state.head.clone())?;
            match self.apply(
                &mut state,
                author,
                &edit.message,
                edit.ops,
                Source::Openplan,
            )? {
                Applied::Nothing => return Ok(None),
                Applied::Moved => continue,
                Applied::Done(committed) => {
                    self.announce(from, &committed, Origin::Local);
                    return Ok(Some(committed));
                }
            }
        }
        Err(BackendError::Contended)
    }

    fn refresh(&self) -> Result<Option<HeadMoved>, BackendError> {
        let mut state = self.lock();
        self.absorb(&mut state)
    }

    // Takes in the writes of other processes and the hand edits on disk.
    fn absorb(&self, state: &mut State) -> Result<Option<HeadMoved>, BackendError> {
        let mut elsewhere = None;
        for _ in 0..WRITE_ATTEMPTS {
            elsewhere = self.catch_up(state)?.or(elsewhere);
            let from = state.head.revision().cloned();
            let ops = self.hand_edits(&state.head)?;
            let author = self.external_author.clone();
            match self.apply(state, &author, EXTERNAL_MESSAGE, ops, Source::Disk)? {
                Applied::Nothing => return Ok(elsewhere),
                Applied::Moved => continue,
                Applied::Done(committed) => {
                    return Ok(Some(self.announce(from, &committed, Origin::External)));
                }
            }
        }
        Err(BackendError::Contended)
    }

    fn hand_edits(&self, head: &MemorySnapshot) -> Result<Vec<Op>, BackendError> {
        let disk = disk::scan(&self.root)?;
        let mut ops = Vec::new();
        for (path, bytes) in &disk {
            if check_path(path).is_err() {
                tracing::warn!(root = %self.root.display(), %path, "a file with this name cannot be a document");
                continue;
            }
            if head.read(path)?.as_ref() != Some(bytes) {
                ops.push(Op::put(path.clone(), bytes.clone()));
            }
        }
        for path in head.files()? {
            if !disk.contains_key(&path) {
                ops.push(Op::remove(path));
            }
        }
        Ok(ops)
    }

    fn apply(
        &self,
        state: &mut State,
        author: &Actor,
        message: &str,
        ops: Vec<Op>,
        source: Source,
    ) -> Result<Applied, BackendError> {
        let mut writes: BTreeMap<String, Option<Vec<u8>>> = BTreeMap::new();
        for op in ops {
            check_path(op.path())?;
            match op {
                Op::Put { path, bytes } => writes.insert(path, Some(bytes)),
                Op::Remove { path } => writes.insert(path, None),
            };
        }
        let mut kinds = BTreeMap::new();
        for (path, content) in &writes {
            let kind = match (state.head.read(path)?, content) {
                (None, Some(_)) => ChangeKind::Added,
                (Some(_), None) => ChangeKind::Removed,
                (Some(old), Some(new)) if old != *new => ChangeKind::Modified,
                _ => continue,
            };
            kinds.insert(path.clone(), kind);
        }
        writes.retain(|path, _| kinds.contains_key(path));
        if writes.is_empty() {
            return Ok(Applied::Nothing);
        }
        let expected = match state.head.revision() {
            Some(id) => Some(parse_id(id)?),
            None => None,
        };
        let Some(recorded) = state.history.record(
            expected,
            author,
            op_backend::now(),
            message,
            &writes,
            &kinds,
        )?
        else {
            return Ok(Applied::Moved);
        };
        if let Source::Openplan = source {
            for (path, content) in &writes {
                disk::write(&self.root, path, content.as_deref())?;
            }
        }
        state.history.settle(parse_id(&recorded.revision.id)?)?;
        let mut files = MemorySnapshot::copy_of(&*state.head)?.into_files();
        for (path, content) in writes {
            match content {
                Some(bytes) => files.insert(path, bytes),
                None => files.remove(&path),
            };
        }
        state.head = Arc::new(MemorySnapshot::new(
            Some(recorded.revision.id.clone()),
            files,
        ));
        Ok(Applied::Done(Committed {
            revision: recorded.revision,
            changes: recorded.changes,
        }))
    }

    fn announce(
        &self,
        from: Option<RevisionId>,
        committed: &Committed,
        origin: Origin,
    ) -> HeadMoved {
        let moved = HeadMoved {
            from,
            to: committed.revision.clone(),
            changes: committed.changes.clone(),
            origin,
        };
        self.events.send(BackendEvent::HeadMoved(moved.clone()));
        moved
    }
}

enum Applied {
    Nothing,
    Moved,
    Done(Committed),
}

impl Backend for LocalBackend {
    fn head(&self) -> Result<Arc<dyn Snapshot>, BackendError> {
        let mut state = self.inner.lock();
        self.inner.catch_up(&mut state)?;
        Ok(state.head.clone())
    }

    fn at(&self, revision: &RevisionId) -> Result<Arc<dyn Snapshot>, BackendError> {
        let id = parse_id(revision)?;
        let state = self.inner.lock();
        if !state.history.exists(id)? {
            return Err(BackendError::UnknownRevision(revision.clone()));
        }
        Ok(Arc::new(state.history.snapshot(Some(id))?))
    }

    fn commit(&self, author: &Actor, write: Write<'_>) -> Result<Option<Committed>, BackendError> {
        self.inner.commit(author, write)
    }

    fn log(&self, query: &LogQuery) -> Result<Vec<LogEntry>, BackendError> {
        self.inner.lock().history.log(query)
    }

    fn refresh(&self) -> Result<Option<HeadMoved>, BackendError> {
        self.inner.refresh()
    }

    fn remote(&self) -> Option<&dyn Remote> {
        None
    }

    fn subscribe(&self) -> broadcast::Receiver<BackendEvent> {
        self.inner.events.subscribe()
    }
}
