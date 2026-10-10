use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, RwLock, Weak};
use std::time::Duration;

use op_api::{BackendKind, ChangeEvent, Fault, FaultKind, Forge, ProjectView, Rfc3339, SyncView};
use op_backend::{
    Actor, Backend, BackendError, BackendEvent, Change, HeadMoved, LogQuery, Origin, RevisionId,
    Schedule, Signer, SyncLoop, SyncStatus,
};
use op_backend_git::GitBackend;
use op_backend_local::LocalBackend;
use op_index::Index;
use op_task::layout::{self, Document};
use op_tracker::{
    FORMATS, FormatError, Formats, HistoryQuery, Stored, TaskMergePolicy, Tracker, TrackerError,
};
use tokio::sync::broadcast::error::RecvError;

use crate::{Publisher, SelfUpdate};

pub const STORE_DIR: &str = ".plan";

// How far back the first load looks for the first and the last change of each task. A task changed
// longer ago than this reads as undated, and one created longer ago reads with no author until it
// changes again.
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
    signer: &Signer,
    watch: bool,
) -> Result<Arc<dyn Backend>, OpenError> {
    open_backend_with(location, signer, watch, TaskMergePolicy::default())
}

fn open_backend_with(
    location: &Location,
    signer: &Signer,
    watch: bool,
    policy: TaskMergePolicy,
) -> Result<Arc<dyn Backend>, OpenError> {
    Ok(match location.kind {
        BackendKind::Git => Arc::new(GitBackend::open(
            &location.root,
            op_backend_git::Options::new(Arc::new(policy), signer.clone()),
        )?),
        BackendKind::Local => Arc::new(LocalBackend::open(
            location.root.join(STORE_DIR),
            op_backend_local::Options {
                watch,
                signer: signer.clone(),
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

#[derive(Debug, Default)]
struct Health {
    root_gone: bool,
    unreadable: Option<String>,
    format: Option<FormatError>,
    unsigned: bool,
    outside_unread: Option<String>,
}

pub struct Project {
    name: RwLock<String>,
    pub path: PathBuf,
    location: Location,
    // Signs what the daemon writes on its own: merges, hand edits, and edits from the web UI.
    signer: Signer,
    tracker: Tracker,
    index: Mutex<Index>,
    loaded: Mutex<Option<RevisionId>>,
    sync: Mutex<Option<SyncLoop>>,
    health: Mutex<Health>,
    // The format problem that stopped the last sync, so a merge with tasks this daemon cannot read.
    sync_format: Arc<Mutex<Option<FormatError>>>,
    updates: OnceLock<Arc<SelfUpdate>>,
    root_misses: AtomicU32,
}

impl Project {
    pub fn open(name: impl Into<String>, location: Location) -> Result<Self, OpenError> {
        Self::open_in(name, location, &FORMATS)
    }

    pub fn open_in(
        name: impl Into<String>,
        location: Location,
        formats: &'static Formats,
    ) -> Result<Self, OpenError> {
        let signer = op_backend_git::signer(&location.root);
        let sync_format: Arc<Mutex<Option<FormatError>>> = Arc::default();
        let reported = Arc::clone(&sync_format);
        let policy = TaskMergePolicy::default()
            .with_formats(formats)
            .on_unreadable(move |problem| {
                *reported.lock().expect("sync format poisoned") = Some(problem.clone());
            });
        let backend = open_backend_with(&location, &signer, true, policy)?;
        let forge = crate::forge::of_project(&location);
        let project = Self {
            name: RwLock::new(name.into()),
            path: location.root.clone(),
            location,
            signer,
            tracker: Tracker::new(backend)
                .with_forge(forge.clone())
                .with_formats(formats),
            index: Mutex::new(Index::new().with_forge(forge)),
            loaded: Mutex::new(None),
            sync: Mutex::new(None),
            health: Mutex::new(Health::default()),
            sync_format,
            updates: OnceLock::new(),
            root_misses: AtomicU32::new(0),
        };
        project.reload();
        Ok(project)
    }

    // The event pump holds the project weakly, so it ends with the backend's event channel once the
    // project is dropped.
    pub(crate) fn start(self: &Arc<Self>, publisher: Publisher) {
        let _ = self.updates.set(Arc::clone(publisher.updates()));
        if self.lock_health().format.as_ref().is_some_and(needs_update) {
            publisher.updates().want();
        }
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

    pub fn forge(&self) -> Option<&Forge> {
        self.tracker.forge()
    }

    // The git identity of the project, read now, so a name set after the project opened signs the
    // next write. Whether it is set is a fault of the project.
    pub fn sign(&self) -> Result<Actor, BackendError> {
        let signed = self.signer.sign();
        self.lock_health().unsigned = matches!(signed, Err(BackendError::NoIdentity));
        signed
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
            sync: self.sync_status().as_ref().map(sync_view),
            forge: self.forge().cloned(),
        }
    }

    // Reads every task again and dates each one from the log.
    pub fn reload(&self) {
        let migration = self.migrate_if_due();
        let result = self.load(Scope::Everything);
        self.record_health(result, migration);
    }

    // Reads only what moved between the revision the index holds and the head, and nothing when
    // the head did not move. A write and the event pump both call it for the same move.
    pub(crate) fn catch_up(&self) {
        let migration = self.migrate_if_due();
        let result = self.load(Scope::Moved);
        self.record_health(result, migration);
    }

    // A store of an older format moves to this daemon's format as soon as the daemon reads it, when
    // a release reads that format. Returns why it could not.
    fn migrate_if_due(&self) -> Option<String> {
        let Ok(Some(Stored::Older(from))) = self.tracker.stored() else {
            return None;
        };
        if !self.tracker.formats().migrates_by_itself() {
            return None;
        }
        let migrated = self
            .sign()
            .map_err(TrackerError::from)
            .and_then(|actor| self.tracker.migrate(&actor));
        match migrated {
            Ok(_) => {
                tracing::info!(project = %self.name(), from, to = self.tracker.formats().current(), "migrated the tasks");
                None
            }
            Err(err) => Some(format!(
                "cannot migrate the tasks from format {from}: {err}"
            )),
        }
    }

    fn record_health(&self, result: Result<Loaded, TrackerError>, migration: Option<String>) {
        let (unreadable, format) = match result {
            Ok(loaded) => (migration.or(loaded.unreadable), loaded.format),
            Err(err) => (Some(err.to_string()), None),
        };
        if let Some(reason) = &unreadable {
            tracing::warn!(project = %self.name(), %reason, "the project cannot serve its tasks");
        }
        if let Some(problem) = &format {
            tracing::warn!(project = %self.name(), %problem, "the tasks use another store format");
        }
        let newly_needs_update = {
            let mut health = self.lock_health();
            let newly = format.as_ref().is_some_and(needs_update) && health.format != format;
            health.unreadable = unreadable;
            health.format = format;
            newly
        };
        if newly_needs_update && let Some(updates) = self.updates.get() {
            updates.want();
        }
    }

    // A store this daemon cannot read; one in an older format that it does not migrate by itself
    // still serves its tasks.
    pub(crate) fn unreadable_format(&self) -> Option<FormatError> {
        self.lock_health()
            .format
            .clone()
            .filter(|problem| !matches!(problem, FormatError::Unmigrated { .. }))
    }

    // A sync that stopped on a format this daemon cannot read asks for an update, as an open does.
    pub(crate) fn synced(&self, status: &op_backend::SyncStatus) {
        let mut sync_format = self.lock_sync_format();
        if status.error.is_none() {
            *sync_format = None;
        }
        if sync_format.is_some()
            && let Some(updates) = self.updates.get()
        {
            updates.want();
        }
    }

    // The head is read under the index lock: two writes that reload at once could otherwise finish
    // in the other order and leave the index on the older head.
    fn load(&self, scope: Scope) -> Result<Loaded, TrackerError> {
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
                index.load(&plan)?;
                for directory in [layout::TASKS, layout::DOCS] {
                    let log = self.tracker.backend().log(&LogQuery {
                        prefix: format!("{directory}/"),
                        before: None,
                        limit: Some(DATING_BUDGET),
                    })?;
                    index.date(&log);
                    index.credit(&log);
                }
            }
            Some(changes) if changes.is_empty() => {}
            Some(changes) => {
                let numbers = task_numbers(&changes);
                let names = doc_names(&changes);
                let dated = self.last_changed(&numbers)?;
                let docs_dated = self.docs_last_changed(&names)?;
                match changes.iter().any(|change| change.path == layout::CONFIG) {
                    true => index.load(&plan)?,
                    false => {
                        index.update(&plan, &numbers)?;
                        index.update_docs(&plan, &names)?;
                    }
                }
                for (number, at) in dated {
                    index.touch(number, at);
                }
                let created = op_index::created(&changes);
                for number in numbers {
                    if index.contains(number)
                        && (created.contains(&number) || !index.credited(number))
                    {
                        let history = self
                            .tracker
                            .task_history(number, &HistoryQuery::default())?;
                        index.credit(&history);
                    }
                }
                for (name, at) in docs_dated {
                    index.touch_doc(&name, at);
                }
                index.carry_doc_authors(&changes);
                for name in op_tracker::doc_moves(&changes).created {
                    if index.doc_exists(&name) && !index.doc_credited(&name) {
                        let history = self.tracker.doc_history(&name, &HistoryQuery::default())?;
                        index.credit(&history);
                    }
                }
            }
        }
        *loaded = head;
        let formats = self.tracker.formats();
        let format = plan.format_problem().cloned().or_else(|| {
            plan.migrated_from()
                .filter(|_| !formats.migrates_by_itself())
                .map(|from| formats.unmigrated(from))
        });
        Ok(Loaded {
            unreadable: match format {
                Some(_) => None,
                None => plan.config().err().map(|err| err.to_string()),
            },
            format,
        })
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

    fn docs_last_changed(
        &self,
        names: &BTreeSet<String>,
    ) -> Result<Vec<(String, op_backend::Timestamp)>, TrackerError> {
        let mut dated = Vec::new();
        for name in names {
            let last = self.tracker.doc_history(
                name,
                &HistoryQuery {
                    before: None,
                    limit: Some(1),
                },
            )?;
            if let Some(entry) = last.first() {
                dated.push((name.clone(), entry.revision.at));
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
                Document::Doc(name) => publisher.publish(ChangeEvent::DocChanged {
                    project: project.clone(),
                    name,
                }),
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
        self.blocked_by(&self.lock_health())
    }

    fn blocked_by(&self, health: &Health) -> Option<String> {
        health
            .root_gone
            .then(|| format!("the project root {} no longer exists", self.path.display()))
    }

    pub fn faults(&self) -> Vec<Fault> {
        let project = self.name();
        let health = self.lock_health();
        let fault = |kind, message: String| Fault {
            project: project.clone(),
            kind,
            message,
        };
        // Nothing else of a project with no root can be read, so nothing else is worth saying.
        if let Some(reason) = self.blocked_by(&health) {
            return vec![fault(FaultKind::RootGone, reason)];
        }
        let mut faults = Vec::new();
        if let Some(reason) = &health.unreadable {
            faults.push(fault(FaultKind::Unreadable, reason.clone()));
        }
        if let Some(problem) = &health.format {
            faults.push(self.format_fault(&project, problem));
        }
        let sync_format = self.lock_sync_format().clone();
        if let Some(problem) = sync_format.as_ref().filter(|_| health.format.is_none()) {
            faults.push(self.format_fault(&project, problem));
        }
        if health.unsigned {
            faults.push(fault(
                FaultKind::NoIdentity,
                BackendError::NoIdentity.to_string(),
            ));
        }
        if let Some(error) = self
            .sync_status()
            .and_then(|status| status.error)
            .filter(|_| sync_format.is_none())
        {
            faults.push(fault(
                FaultKind::SyncFailed,
                format!("the last sync failed: {error}"),
            ));
        }
        if let Some(error) = &health.outside_unread {
            faults.push(fault(
                FaultKind::OutsideChangesUnread,
                format!("changes made outside openplan were not read: {error}"),
            ));
        }
        faults
    }

    // One watchdog tick: whether the root still exists, whether git names someone to sign a write,
    // and any write made outside this daemon. Answers whether the project started or stopped
    // answering.
    pub fn poll(&self) -> bool {
        let unsigned = matches!(self.signer.sign(), Err(BackendError::NoIdentity));
        let outside_unread = match self.tracker.backend().refresh() {
            Ok(_) | Err(BackendError::NoIdentity) => None,
            Err(err) => {
                tracing::warn!(project = %self.name(), error = %err, "outside changes not read");
                Some(err.to_string())
            }
        };
        let misses = match self.path.is_dir() {
            true => {
                self.root_misses.store(0, Ordering::Relaxed);
                0
            }
            false => self.root_misses.fetch_add(1, Ordering::Relaxed) + 1,
        };
        let gone = misses >= ROOT_MISSES;
        let mut health = self.lock_health();
        health.unsigned = unsigned;
        health.outside_unread = outside_unread;
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

    fn format_fault(&self, project: &str, problem: &FormatError) -> Fault {
        let (kind, message) = match problem {
            FormatError::Newer { requires, .. } => (
                FaultKind::NewerFormat,
                format!("{problem}; {}", self.update_hint(requires.as_deref())),
            ),
            FormatError::Retired { .. } | FormatError::Unmigrated { .. } => {
                (FaultKind::OlderFormat, problem.to_string())
            }
        };
        Fault {
            project: project.to_owned(),
            kind,
            message,
        }
    }

    // Only a canary reads a format that no release reads yet, and its version says so.
    fn update_hint(&self, requires: Option<&str>) -> String {
        let command = match requires.is_some_and(|version| version.contains('-')) {
            true => "`openplan update --canary`",
            false => "`openplan update`",
        };
        let updates = self.updates.get();
        match (
            updates.and_then(|updates| updates.outcome()),
            updates.is_some_and(|updates| updates.automatic()),
        ) {
            (Some(outcome), _) => {
                format!("the update check installed no newer openplan ({outcome}); run {command}")
            }
            (None, true) => "the daemon is looking for an update now".to_owned(),
            (None, false) => format!("run {command}"),
        }
    }

    fn lock_sync_format(&self) -> MutexGuard<'_, Option<FormatError>> {
        self.sync_format.lock().expect("sync format poisoned")
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

struct Loaded {
    unreadable: Option<String>,
    format: Option<FormatError>,
}

fn needs_update(problem: &FormatError) -> bool {
    matches!(problem, FormatError::Newer { .. })
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

fn doc_names(changes: &[Change]) -> BTreeSet<String> {
    changes
        .iter()
        .filter_map(|change| layout::doc_name(&change.path).map(str::to_owned))
        .collect()
}

pub fn sync_view(status: &SyncStatus) -> SyncView {
    SyncView {
        remote: status.remote.clone(),
        last_attempt: status.last_attempt.map(Rfc3339),
        last_success: status.last_success.map(Rfc3339),
        ahead: status.ahead,
        behind: status.behind,
        syncing: status.syncing,
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
                publisher.report(&project);
                continue;
            }
            Err(RecvError::Closed) => return,
        };
        let Some(project) = project.upgrade() else {
            return;
        };
        match event {
            BackendEvent::HeadMoved(moved) => project.moved(&moved, &publisher),
            BackendEvent::Sync(status) => {
                project.synced(&status);
                publisher.publish(ChangeEvent::SyncChanged {
                    project: project.name(),
                })
            }
        }
        publisher.report(&project);
    }
}
