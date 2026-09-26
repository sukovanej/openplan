use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::SystemTime;

use op_backend::{
    Actor, Backend, BackendError, BackendEvent, Change, ChangeKind, Committed, Events, HeadMoved,
    LogEntry, LogQuery, Op, Origin, Remote, RevisionId, Snapshot, Write, check_path,
};
use tokio::sync::broadcast;

mod disk;
mod history;
mod snapshot;
mod watch;

use disk::Stamp;
use history::{History, parse_id};
use snapshot::{Blobs, LocalSnapshot, Stored};

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
    blobs: Arc<Blobs>,
    state: Mutex<State>,
    events: Events,
}

struct State {
    history: History,
    head: Arc<LocalSnapshot>,
    // Files that held what the head holds when these stamps were taken, so a scan reads only the
    // files whose stamp moved.
    stamps: BTreeMap<String, Stamp>,
}

struct HandEdits {
    ops: Vec<Op>,
    settled: Vec<(String, Stamp)>,
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
        let blobs = Arc::new(Blobs::open(&root.join(HISTORY_FILE))?);
        // Another open of the same store can record a revision at any moment and settle it a moment
        // later. The head is read after the unsettled revisions, so it holds each of them, and
        // finishing one writes its files, not the older versions from before it.
        let unsettled = history.unsettled()?;
        let head = Arc::new(history.snapshot(history.latest()?, &blobs)?);
        let inner = Arc::new(Inner {
            root,
            external_author: options.external_author,
            blobs,
            state: Mutex::new(State {
                history,
                head,
                stamps: BTreeMap::new(),
            }),
            events: Events::default(),
        });
        inner.finish_interrupted(&unsettled)?;
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

    fn finish_interrupted(&self, unsettled: &[i64]) -> Result<(), BackendError> {
        let state = self.lock();
        for &id in unsettled {
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
        let from = state.head.revision().cloned();
        let held = from.as_ref().map(parse_id).transpose()?;
        let (changes, next) = match latest {
            Some(latest) if held.is_none_or(|held| held < latest) => {
                let changes = state.history.changes(held, latest)?;
                let mut writes = Vec::with_capacity(changes.len());
                for change in &changes {
                    state.stamps.remove(&change.path);
                    let stored = state.history.stored_at(latest, &change.path)?;
                    writes.push((change.path.clone(), stored));
                }
                let next = state.head.with(history::revision_id(latest), writes);
                (changes, next)
            }
            // A history that went back names no line of moves from the head, so read it whole.
            _ => {
                let next = state.history.snapshot(latest, &self.blobs)?;
                state.stamps.clear();
                (op_backend::diff(&*state.head, &next)?, next)
            }
        };
        state.head = Arc::new(next);
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
            let edits = self.hand_edits(state)?;
            let author = self.external_author.clone();
            let applied = self.apply(state, &author, EXTERNAL_MESSAGE, edits.ops, Source::Disk)?;
            if !matches!(applied, Applied::Moved) {
                state.stamps.extend(edits.settled);
            }
            match applied {
                Applied::Nothing => return Ok(elsewhere),
                Applied::Moved => continue,
                Applied::Done(committed) => {
                    return Ok(Some(self.announce(from, &committed, Origin::External)));
                }
            }
        }
        Err(BackendError::Contended)
    }

    // Reads only the files whose stamp moved since they last matched the head.
    fn hand_edits(&self, state: &mut State) -> Result<HandEdits, BackendError> {
        let now = SystemTime::now();
        let listed = disk::list(&self.root)?;
        state.stamps.retain(|path, _| listed.contains_key(path));
        let mut edits = HandEdits {
            ops: Vec::new(),
            settled: Vec::new(),
        };
        for (path, stamp) in &listed {
            if check_path(path).is_err() {
                tracing::warn!(root = %self.root.display(), %path, "a file with this name cannot be a document");
                continue;
            }
            if state.stamps.get(path) == Some(stamp) {
                continue;
            }
            let Some(bytes) = disk::read(&self.root, path)? else {
                continue;
            };
            if stamp.is_settled(now) {
                edits.settled.push((path.clone(), *stamp));
            }
            if !state.head.holds(path, &bytes) {
                edits.ops.push(Op::put(path.clone(), bytes));
            }
        }
        for path in state.head.files()? {
            if !listed.contains_key(&path) {
                edits.ops.push(Op::remove(path));
            }
        }
        Ok(edits)
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
            let kind = match (state.head.contains(path), content) {
                (false, Some(_)) => ChangeKind::Added,
                (true, None) => ChangeKind::Removed,
                (true, Some(new)) if !state.head.holds(path, new) => ChangeKind::Modified,
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
                state.stamps.remove(path);
            }
        }
        state.history.settle(parse_id(&recorded.revision.id)?)?;
        let writes = writes
            .into_iter()
            .map(|(path, content)| (path, content.map(Stored::of)));
        state.head = Arc::new(state.head.with(recorded.revision.id.clone(), writes));
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
        Ok(Arc::new(
            state.history.snapshot(Some(id), &self.inner.blobs)?,
        ))
    }

    fn read_at(&self, revision: &RevisionId, path: &str) -> Result<Option<Vec<u8>>, BackendError> {
        let id = parse_id(revision)?;
        let stored = {
            let state = self.inner.lock();
            if !state.history.exists(id)? {
                return Err(BackendError::UnknownRevision(revision.clone()));
            }
            state.history.stored_at(id, path)?
        };
        stored
            .map(|stored| self.inner.blobs.bytes(&stored))
            .transpose()
    }

    fn commit(&self, author: &Actor, write: Write<'_>) -> Result<Option<Committed>, BackendError> {
        self.inner.commit(author, write)
    }

    fn log(&self, query: &LogQuery) -> Result<Vec<LogEntry>, BackendError> {
        self.inner.lock().history.log(query)
    }

    fn changes(
        &self,
        from: Option<&RevisionId>,
        to: &RevisionId,
    ) -> Result<Vec<Change>, BackendError> {
        let state = self.inner.lock();
        for revision in from.into_iter().chain([to]) {
            if !state.history.exists(parse_id(revision)?)? {
                return Err(BackendError::UnknownRevision(revision.clone()));
            }
        }
        state
            .history
            .changes(from.map(parse_id).transpose()?, parse_id(to)?)
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
