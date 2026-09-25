use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, RwLock, Weak};
use std::time::Duration;

use op_api::{BackendKind, ChangeEvent, ProjectStatus, ProjectView, Rfc3339, SyncView};
use op_backend::{
    Actor, Backend, BackendError, BackendEvent, Change, HeadMoved, LogQuery, Origin, RevisionId,
    Schedule, SyncLoop, SyncStatus,
};
use op_backend_git::GitBackend;
use op_backend_local::LocalBackend;
use op_index::Index;
use op_task::layout::{self, Document};
use op_tracker::{HistoryQuery, TaskMergePolicy, Tracker, TrackerError};
use tokio::sync::broadcast::error::RecvError;

use crate::Publisher;

pub const STORE_DIR: &str = ".plan";

// How far back the first load looks for the last change of each task. A task changed longer ago
// than this reads as undated.
const DATING_BUDGET: usize = 4096;
// Two misses in sequence, so a root that reads as absent for a moment does not demote the project.
const ROOT_MISSES: u32 = 2;
pub const ROOT_POLL: Duration = Duration::from_secs(5);

#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    #[error("no tasks at {0}; start them with `openplan init --abbreviation <ABC>`")]
    NoStore(PathBuf),
    #[error(
        "{0} keeps its tasks in {STORE_DIR}/ beside the code; move them out with `openplan migrate`"
    )]
    NeedsMigration(PathBuf),
    #[error("{0} is not in a git repository, so it cannot keep its tasks on a git branch")]
    NoRepo(PathBuf),
    #[error(transparent)]
    Tracker(#[from] TrackerError),
    #[error(transparent)]
    Backend(#[from] BackendError),
}

// Where a project's tasks live, found before anything opens them. `root` is the main checkout of a
// git project, or the directory that holds `.plan` for a local one. Every worktree of a repository
// resolves to one location, so they all see the same tasks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub kind: BackendKind,
    pub root: PathBuf,
    pub git_common_dir: Option<PathBuf>,
}

impl Location {
    // `requested` is what the caller asked for; `None` takes what the path already holds.
    pub fn find(path: &Path, requested: Option<BackendKind>) -> Result<Self, OpenError> {
        let path = &canonical(path);
        let checkout = op_backend_git::inspect(path);
        let in_git = |checkout: &op_backend_git::Checkout| Location {
            kind: BackendKind::Git,
            root: canonical(checkout.root.as_deref().unwrap_or(path)),
            git_common_dir: Some(canonical(&checkout.common_dir)),
        };
        let local = |root: &Path| Location {
            kind: BackendKind::Local,
            root: canonical(root),
            git_common_dir: None,
        };
        match requested {
            Some(BackendKind::Git) => checkout
                .as_ref()
                .map(in_git)
                .ok_or_else(|| OpenError::NoRepo(canonical(path))),
            Some(BackendKind::Local) => Ok(local(&store_root(path).unwrap_or(path.to_path_buf()))),
            None => {
                if let Some(checkout) = checkout.as_ref().filter(|checkout| checkout.has_tasks) {
                    return Ok(in_git(checkout));
                }
                if let Some(root) = local_root(path) {
                    return Ok(local(&root));
                }
                let workdir = checkout
                    .as_ref()
                    .and_then(|checkout| checkout.workdir.as_deref())
                    .map(canonical);
                if let Some(root) = workdir.and_then(|workdir| code_root(path, &workdir)) {
                    return Err(OpenError::NeedsMigration(root));
                }
                match store_root(path) {
                    Some(root) => Ok(local(&root)),
                    None => Err(OpenError::NoStore(canonical(path))),
                }
            }
        }
    }

    // A clone holds no tasks until it fetches them: git fetches no reference outside `refs/heads/`
    // and `refs/tags/` by itself. So a checkout with no tasks asks its remote once.
    pub fn find_or_join(path: &Path) -> Result<Self, OpenError> {
        match Self::find(path, None) {
            Err(OpenError::NoStore(_)) if fetched_from_remote(path) => Self::find(path, None),
            found => found,
        }
    }

    pub fn key(&self) -> &Path {
        self.git_common_dir.as_deref().unwrap_or(&self.root)
    }
}

fn fetched_from_remote(path: &Path) -> bool {
    if op_backend_git::inspect(path).is_none() {
        return false;
    }
    match op_backend_git::fetch_tasks(path, op_backend_git::DEFAULT_REMOTE) {
        Ok(fetched) => fetched,
        Err(err) => {
            tracing::warn!(path = %path.display(), error = %err, "cannot fetch the tasks from the remote");
            false
        }
    }
}

// `watch` picks up hand edits of a local directory as they happen; a one-shot caller reads them on
// open instead.
pub fn open_backend(
    location: &Location,
    machine: &Actor,
    watch: bool,
) -> Result<Arc<dyn Backend>, OpenError> {
    Ok(match location.kind {
        BackendKind::Git => {
            let policy = Arc::new(TaskMergePolicy);
            let mut options = op_backend_git::Options::new(policy);
            options.machine = machine.clone();
            Arc::new(GitBackend::open(&location.root, options)?)
        }
        BackendKind::Local => Arc::new(LocalBackend::open(
            location.root.join(STORE_DIR),
            op_backend_local::Options {
                watch,
                external_author: machine.clone(),
            },
        )?),
    })
}

fn local_root(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .find(|dir| holds_history(dir))
        .map(Path::to_path_buf)
}

// A store copied by hand has no history yet, but its config marks it all the same. The daemon's
// home is also named `.plan` (`~/.plan`), and it holds neither.
fn store_root(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .find(|dir| holds_history(dir) || holds_config(dir))
        .map(Path::to_path_buf)
}

// The directory that keeps its tasks in `.plan/` beside the code, as before the tasks moved out.
fn code_root(path: &Path, workdir: &Path) -> Option<PathBuf> {
    path.ancestors()
        .take_while(|dir| dir.starts_with(workdir))
        .find(|dir| holds_config(dir))
        .map(canonical)
}

fn holds_history(dir: &Path) -> bool {
    dir.join(STORE_DIR)
        .join(op_backend_local::HISTORY_FILE)
        .is_file()
}

fn holds_config(dir: &Path) -> bool {
    dir.join(STORE_DIR).join(layout::CONFIG).is_file()
}

pub(crate) fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

// The name that signs what the daemon writes on its own: merges, sync notes, and edits from the
// web UI. The repository's git identity where there is one.
pub fn machine_actor(root: &Path) -> Actor {
    op_backend_git::identity(root).unwrap_or_else(|| {
        Actor::new(
            std::env::var("USER")
                .ok()
                .filter(|user| !user.trim().is_empty())
                .unwrap_or_else(|| "openplan".to_owned()),
        )
    })
}

#[derive(Debug, Default)]
struct Health {
    root_gone: bool,
    error: Option<String>,
}

pub struct Project {
    name: RwLock<String>,
    pub path: PathBuf,
    location: Location,
    machine: Actor,
    tracker: Tracker,
    index: Mutex<Index>,
    loaded: Mutex<Option<RevisionId>>,
    sync: Mutex<Option<SyncLoop>>,
    health: Mutex<Health>,
    root_misses: AtomicU32,
}

impl Project {
    pub fn open(name: impl Into<String>, location: Location) -> Result<Self, OpenError> {
        let machine = machine_actor(&location.root);
        let backend = open_backend(&location, &machine, true)?;
        let project = Self {
            name: RwLock::new(name.into()),
            path: location.root.clone(),
            location,
            machine,
            tracker: Tracker::new(backend),
            index: Mutex::new(Index::new()),
            loaded: Mutex::new(None),
            sync: Mutex::new(None),
            health: Mutex::new(Health::default()),
            root_misses: AtomicU32::new(0),
        };
        project.reload();
        Ok(project)
    }

    // The event pump holds the project weakly, so it ends with the backend's event channel once the
    // project is dropped.
    pub(crate) fn start(self: &Arc<Self>, publisher: Publisher) {
        let events = self.tracker.backend().subscribe();
        let project = Arc::downgrade(self);
        std::thread::spawn(move || pump(project, events, publisher));
        *self.lock_sync() =
            SyncLoop::start(Arc::clone(self.tracker.backend()), Schedule::default());
    }

    pub fn stop(&self) {
        let sync = self.lock_sync().take();
        drop(sync);
    }

    pub fn name(&self) -> String {
        self.name.read().expect("name lock poisoned").clone()
    }

    pub fn rename(&self, name: &str) {
        *self.name.write().expect("name lock poisoned") = name.to_owned();
    }

    pub fn location(&self) -> &Location {
        &self.location
    }

    pub fn kind(&self) -> BackendKind {
        self.location.kind
    }

    pub fn tracker(&self) -> &Tracker {
        &self.tracker
    }

    pub fn machine(&self) -> &Actor {
        &self.machine
    }

    pub fn index(&self) -> MutexGuard<'_, Index> {
        self.index.lock().expect("index mutex poisoned")
    }

    pub fn sync_status(&self) -> Option<SyncStatus> {
        self.tracker
            .backend()
            .remote()
            .map(|remote| remote.status())
    }

    pub fn sync_soon(&self) {
        if let Some(sync) = self.lock_sync().as_ref() {
            sync.sync_soon();
        }
    }

    pub fn view(&self) -> ProjectView {
        ProjectView {
            name: self.name(),
            root: self.path.display().to_string(),
            git_common_dir: self
                .location
                .git_common_dir
                .as_ref()
                .map(|dir| dir.display().to_string()),
            backend: self.location.kind,
            abbreviation: self
                .index()
                .abbreviation()
                .map(|abbreviation| abbreviation.to_string())
                .unwrap_or_default(),
            status: self.status(),
            sync: self.sync_status().as_ref().map(sync_view),
        }
    }

    // Reads every task again and dates each one from the log.
    pub fn reload(&self) {
        let result = self.load(Scope::Everything);
        self.record_health(result);
    }

    // Reads only what moved between the revision the index holds and the head, and nothing when
    // the head did not move. A write and the event pump both call it for the same move.
    pub(crate) fn catch_up(&self) {
        let result = self.load(Scope::Moved);
        self.record_health(result);
    }

    fn record_health(&self, result: Result<Option<String>, TrackerError>) {
        let error = match &result {
            Ok(plan_error) => plan_error.clone(),
            Err(err) => Some(err.to_string()),
        };
        if let Some(reason) = &error {
            tracing::warn!(project = %self.name(), %reason, "the project cannot serve its tasks");
        }
        self.lock_health().error = error;
    }

    // The head is read under the index lock: two writes that reload at once could otherwise finish
    // in the other order and leave the index on the older head.
    fn load(&self, scope: Scope) -> Result<Option<String>, TrackerError> {
        let mut index = self.index();
        let plan = self.tracker.plan()?;
        let mut loaded = self.loaded.lock().expect("loaded mutex poisoned");
        let head = plan.revision().cloned();
        let moved = match (scope, loaded.as_ref(), head.as_ref()) {
            (Scope::Everything, _, _) => None,
            (Scope::Moved, Some(from), Some(to)) if from == to => Some(Vec::new()),
            (Scope::Moved, Some(from), Some(to)) => {
                self.tracker.backend().changes(Some(from), to).ok()
            }
            (Scope::Moved, _, _) => None,
        };
        match moved {
            None => {
                let log = self.tracker.backend().log(&LogQuery {
                    prefix: format!("{}/", layout::TASKS),
                    before: None,
                    limit: Some(DATING_BUDGET),
                })?;
                index.load(&plan)?;
                index.date(&log);
            }
            Some(changes) if changes.is_empty() => {}
            Some(changes) => {
                let numbers = task_numbers(&changes);
                let dated = self.last_changed(&numbers)?;
                match changes.iter().any(|change| change.path == layout::CONFIG) {
                    true => index.load(&plan)?,
                    false => index.update(&plan, &numbers)?,
                }
                for (number, at) in dated {
                    index.touch(number, at);
                }
            }
        }
        *loaded = head;
        Ok(plan.config().err().map(|err| err.to_string()))
    }

    fn last_changed(
        &self,
        numbers: &BTreeSet<u64>,
    ) -> Result<Vec<(u64, op_backend::Timestamp)>, TrackerError> {
        let mut dated = Vec::new();
        for &number in numbers {
            let last = self.tracker.task_history(
                number,
                &HistoryQuery {
                    before: None,
                    limit: Some(1),
                },
            )?;
            if let Some(entry) = last.first() {
                dated.push((number, entry.revision.at));
            }
        }
        Ok(dated)
    }

    fn moved(&self, moved: &HeadMoved, publisher: &Publisher) {
        self.catch_up();
        let project = self.name();
        let (mut tags, mut config) = (false, false);
        let keys: Vec<String> = {
            let index = self.index();
            task_numbers(&moved.changes)
                .into_iter()
                .map(|number| index.key(number))
                .collect()
        };
        for id in keys {
            publisher.publish(ChangeEvent::TaskChanged {
                project: project.clone(),
                id,
            });
        }
        for change in &moved.changes {
            match Document::of(&change.path) {
                Document::Tag(_) => tags = true,
                Document::Config => config = true,
                _ => {}
            }
        }
        if tags {
            publisher.publish(ChangeEvent::TagsChanged {
                project: project.clone(),
            });
        }
        if config {
            publisher.publish(ChangeEvent::ProjectsChanged);
        }
        if moved.origin == Origin::Local {
            self.sync_soon();
        }
    }

    // What stops this project from answering at all. A project with no tasks yet still answers, so
    // `init` can start them.
    pub fn blocked(&self) -> Option<String> {
        self.lock_health()
            .root_gone
            .then(|| format!("the project root {} no longer exists", self.path.display()))
    }

    pub fn status(&self) -> ProjectStatus {
        match self.blocked().or_else(|| self.lock_health().error.clone()) {
            Some(reason) => ProjectStatus::Error { reason },
            None => ProjectStatus::Ok,
        }
    }

    // One watchdog tick: whether the root still exists, and any write made outside this daemon.
    // Answers whether the project's status moved.
    pub fn poll(&self) -> bool {
        if let Err(err) = self.tracker.backend().refresh() {
            tracing::warn!(project = %self.name(), error = %err, "outside changes not read");
        }
        let misses = match self.path.is_dir() {
            true => {
                self.root_misses.store(0, Ordering::Relaxed);
                0
            }
            false => self.root_misses.fetch_add(1, Ordering::Relaxed) + 1,
        };
        let gone = misses >= ROOT_MISSES;
        let mut health = self.lock_health();
        let moved = health.root_gone != gone;
        health.root_gone = gone;
        if moved {
            match gone {
                true => {
                    tracing::warn!(project = %self.name(), root = %self.path.display(), "project root is gone")
                }
                false => {
                    tracing::info!(project = %self.name(), root = %self.path.display(), "project root is back")
                }
            }
        }
        moved
    }

    fn lock_health(&self) -> MutexGuard<'_, Health> {
        self.health.lock().expect("health mutex poisoned")
    }

    fn lock_sync(&self) -> MutexGuard<'_, Option<SyncLoop>> {
        self.sync.lock().expect("sync mutex poisoned")
    }
}

impl std::fmt::Debug for Project {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Project")
            .field("name", &self.name())
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy)]
enum Scope {
    Everything,
    Moved,
}

fn task_numbers(changes: &[Change]) -> BTreeSet<u64> {
    changes
        .iter()
        .filter_map(|change| layout::task_number(&change.path))
        .collect()
}

pub fn sync_view(status: &SyncStatus) -> SyncView {
    SyncView {
        remote: status.remote.clone(),
        last_attempt: status.last_attempt.map(Rfc3339),
        last_success: status.last_success.map(Rfc3339),
        ahead: status.ahead,
        behind: status.behind,
        error: status.error.clone(),
    }
}

fn pump(
    project: Weak<Project>,
    mut events: tokio::sync::broadcast::Receiver<BackendEvent>,
    publisher: Publisher,
) {
    loop {
        let event = match events.blocking_recv() {
            Ok(event) => event,
            Err(RecvError::Lagged(_)) => {
                let Some(project) = project.upgrade() else {
                    return;
                };
                project.reload();
                publisher.publish(ChangeEvent::Resync);
                continue;
            }
            Err(RecvError::Closed) => return,
        };
        let Some(project) = project.upgrade() else {
            return;
        };
        match event {
            BackendEvent::HeadMoved(moved) => project.moved(&moved, &publisher),
            BackendEvent::Sync(_) => publisher.publish(ChangeEvent::SyncChanged {
                project: project.name(),
            }),
        }
    }
}
