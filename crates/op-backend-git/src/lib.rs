use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use gix::ObjectId;
use op_backend::{
    Actor, Backend, BackendError, BackendEvent, Change, Committed, Events, HeadMoved, LogEntry,
    LogQuery, MergePolicy, Origin, Remote, RevisionId, Snapshot, SyncStatus, Write,
};
use tokio::sync::broadcast;

mod checkout;
mod import;
mod objects;
mod sync;

pub use checkout::{Checkout, identity, inspect};
pub use import::{Imported, UNCOMMITTED_MESSAGE};
pub use sync::fetch_tasks;

use objects::{GitSnapshot, object_id, storage};

// Outside `refs/heads/`: a forge lists no branch for the tasks and compares them with no code
// branch. Git still finds the short name, so `git log openplan/tasks` works.
pub const TASKS_REF: &str = "refs/openplan/tasks";
pub const TASKS_NAME: &str = "openplan/tasks";
pub const DEFAULT_REMOTE: &str = "origin";
pub const NETWORK_TIMEOUT: Duration = Duration::from_secs(60);

const WRITE_ATTEMPTS: usize = 64;

pub struct Options {
    // Ignored when the repository has no remote of that name.
    pub remote: Option<String>,
    pub policy: Arc<dyn MergePolicy>,
    // Signs merge commits and reference logs, which no person wrote.
    pub machine: Actor,
    // A half-open connection after a sleep or a network change makes git wait with no end.
    pub network_timeout: Duration,
}

impl Options {
    pub fn new(policy: Arc<dyn MergePolicy>) -> Self {
        Self {
            remote: Some(DEFAULT_REMOTE.to_owned()),
            policy,
            machine: Actor::new("openplan"),
            network_timeout: NETWORK_TIMEOUT,
        }
    }
}

pub struct GitBackend {
    inner: Arc<Inner>,
}

pub(crate) struct Inner {
    repo: gix::ThreadSafeRepository,
    common_dir: PathBuf,
    remote: Option<String>,
    policy: Arc<dyn MergePolicy>,
    machine: Actor,
    network_timeout: Duration,
    writing: Mutex<()>,
    syncing: Mutex<()>,
    announced: Mutex<Option<ObjectId>>,
    cached: Mutex<Option<Arc<GitSnapshot>>>,
    // What each revision changed. A revision never changes, so an answer stays true.
    changed_by: Mutex<HashMap<ObjectId, Arc<[Change]>>>,
    status: Mutex<SyncStatus>,
    events: Events,
}

impl GitBackend {
    pub fn open(path: impl AsRef<Path>, options: Options) -> Result<Self, BackendError> {
        let repo = gix::ThreadSafeRepository::discover(path.as_ref()).map_err(storage)?;
        let local = repo.to_thread_local();
        let common_dir = local.common_dir().to_path_buf();
        let remote = options
            .remote
            .filter(|name| local.find_remote(name.as_str()).is_ok());
        if let Some(remote) = &remote {
            start_from_tracking(&local, remote, &options.machine)?;
        }
        let announced = objects::tip(&local, TASKS_REF)?;
        let status = SyncStatus {
            remote: remote.clone().unwrap_or_default(),
            ..SyncStatus::default()
        };
        Ok(Self {
            inner: Arc::new(Inner {
                repo,
                common_dir,
                remote,
                policy: options.policy,
                machine: options.machine,
                network_timeout: options.network_timeout,
                writing: Mutex::new(()),
                syncing: Mutex::new(()),
                announced: Mutex::new(announced),
                cached: Mutex::new(None),
                changed_by: Mutex::new(HashMap::new()),
                status: Mutex::new(status),
                events: Events::default(),
            }),
        })
    }

    pub fn common_dir(&self) -> &Path {
        &self.inner.common_dir
    }
}

// A repository that fetched the tasks without the daemon, as `fetch_tasks` or a CI job does, holds
// only the remote-tracking reference. Starting the local one there lets a reader see the tasks
// before any sync runs.
fn start_from_tracking(
    repo: &gix::Repository,
    remote: &str,
    machine: &Actor,
) -> Result<(), BackendError> {
    let tracking = tracking_reference(remote);
    if objects::tip(repo, TASKS_REF)?.is_none()
        && let Some(theirs) = objects::tip(repo, &tracking)?
    {
        objects::move_reference(
            repo,
            TASKS_REF,
            None,
            theirs,
            machine,
            &format!("start from {tracking}"),
        )?;
    }
    Ok(())
}

// Outside `refs/remotes/` too: `git fetch --prune` deletes a reference there that no remote branch
// backs.
pub fn tracking_reference(remote: &str) -> String {
    format!("refs/openplan/remotes/{remote}/tasks")
}

impl Inner {
    fn local(&self) -> gix::Repository {
        self.repo.to_thread_local()
    }

    fn tip(&self) -> Result<Option<ObjectId>, BackendError> {
        objects::tip(&self.local(), TASKS_REF)
    }

    fn snapshot(&self, commit: Option<ObjectId>) -> Result<Arc<GitSnapshot>, BackendError> {
        let mut cached = lock(&self.cached);
        if let Some(snapshot) = cached.as_ref()
            && snapshot.revision().map(object_id).transpose()? == commit
        {
            return Ok(Arc::clone(snapshot));
        }
        let snapshot = Arc::new(GitSnapshot::of(&self.repo, commit)?);
        *cached = Some(Arc::clone(&snapshot));
        Ok(snapshot)
    }

    fn commit(&self, author: &Actor, write: Write<'_>) -> Result<Option<Committed>, BackendError> {
        let _writing = lock(&self.writing);
        let repo = self.local();
        for _ in 0..WRITE_ATTEMPTS {
            let tip = self.tip()?;
            self.catch_up(tip)?;
            let head = self.snapshot(tip)?;
            let edit = write(head.clone())?;
            let written = objects::write_tree(&repo, &head, &edit.ops)?;
            if written.changes.is_empty() {
                return Ok(None);
            }
            let at = op_backend::now();
            let parents: Vec<ObjectId> = tip.into_iter().collect();
            let commit =
                objects::write_commit(&repo, written.tree, &parents, author, at, &edit.message)?;
            if !objects::move_reference(&repo, TASKS_REF, tip, commit, author, &edit.message)? {
                objects::pause_before_retry();
                continue;
            }
            let revision = objects::read_revision(&repo, commit)?;
            let committed = Committed {
                revision,
                changes: written.changes,
            };
            self.announce(Some(commit), committed.changes.clone(), Origin::Local)?;
            return Ok(Some(committed));
        }
        Err(BackendError::Contended)
    }

    // Records the head the listeners now know about, and tells them how it moved.
    fn announce(
        &self,
        to: Option<ObjectId>,
        changes: Vec<Change>,
        origin: Origin,
    ) -> Result<Option<HeadMoved>, BackendError> {
        let mut announced = lock(&self.announced);
        let from = *announced;
        *announced = to;
        let Some(to) = to else {
            return Ok(None);
        };
        if from == Some(to) {
            return Ok(None);
        }
        let moved = HeadMoved {
            from: from.map(objects::revision_id),
            to: objects::read_revision(&self.local(), to)?,
            changes,
            origin,
        };
        self.events.send(BackendEvent::HeadMoved(moved.clone()));
        Ok(Some(moved))
    }

    pub(crate) fn changes_between(
        &self,
        from: Option<ObjectId>,
        to: Option<ObjectId>,
    ) -> Result<Vec<Change>, BackendError> {
        let repo = self.local();
        let tree = |commit: Option<ObjectId>| {
            commit
                .map(|commit| objects::tree_of(&repo, commit))
                .transpose()
        };
        objects::tree_changes(&repo, tree(from)?, tree(to)?)
    }

    // A merge keeps only what differs from every side it joined: a document it took unchanged from
    // one side belongs to the revision that made it there.
    fn changed_by(
        &self,
        repo: &gix::Repository,
        commit: ObjectId,
        parents: &[ObjectId],
    ) -> Result<Arc<[Change]>, BackendError> {
        if let Some(known) = lock(&self.changed_by).get(&commit) {
            return Ok(Arc::clone(known));
        }
        let tree = objects::tree_of(repo, commit)?;
        let first = parents
            .first()
            .map(|parent| objects::tree_of(repo, *parent))
            .transpose()?;
        let mut changes = objects::tree_changes(repo, first, Some(tree))?;
        if let Some(others) = parents.get(1..).filter(|others| !others.is_empty()) {
            let mut kept = Vec::with_capacity(changes.len());
            for change in changes {
                let ours = objects::blob_at(repo, tree, &change.path)?;
                let mut differs = true;
                for other in others {
                    let theirs = objects::tree_of(repo, *other)?;
                    differs &= objects::blob_at(repo, theirs, &change.path)? != ours;
                }
                if differs {
                    kept.push(change);
                }
            }
            changes = kept;
        }
        let changes: Arc<[Change]> = changes.into();
        lock(&self.changed_by).insert(commit, Arc::clone(&changes));
        Ok(changes)
    }

    fn snapshot_at(&self, commit: ObjectId) -> Result<Arc<GitSnapshot>, BackendError> {
        if self.tip()? == Some(commit) {
            return self.snapshot(Some(commit));
        }
        Ok(Arc::new(GitSnapshot::of(&self.repo, Some(commit))?))
    }

    fn refresh(&self) -> Result<Option<HeadMoved>, BackendError> {
        let _writing = lock(&self.writing);
        self.catch_up(self.tip()?)
    }

    // Tells the listeners about a move another process made, so the next move they hear about
    // starts where they are.
    pub(crate) fn catch_up(
        &self,
        tip: Option<ObjectId>,
    ) -> Result<Option<HeadMoved>, BackendError> {
        let from = *lock(&self.announced);
        if from == tip {
            return Ok(None);
        }
        let changes = self.changes_between(from, tip)?;
        self.announce(tip, changes, Origin::External)
    }

    fn log(&self, query: &LogQuery) -> Result<Vec<LogEntry>, BackendError> {
        let repo = self.local();
        let starts: Vec<ObjectId> = match &query.before {
            Some(before) => {
                let before = object_id(before)?;
                let commit = repo
                    .find_commit(before)
                    .map_err(|_| BackendError::UnknownRevision(objects::revision_id(before)))?;
                commit.parent_ids().map(|id| id.detach()).collect()
            }
            None => self.tip()?.into_iter().collect(),
        };
        if starts.is_empty() {
            return Ok(Vec::new());
        }
        let walk = repo
            .rev_walk(starts)
            .sorting(gix::revision::walk::Sorting::ByCommitTime(
                gix::traverse::commit::simple::CommitTimeOrder::NewestFirst,
            ))
            .all()
            .map_err(storage)?;
        let mut entries = Vec::new();
        for info in walk {
            if query.limit.is_some_and(|limit| entries.len() >= limit) {
                break;
            }
            let info = info.map_err(storage)?;
            let changes: Vec<Change> = self
                .changed_by(&repo, info.id, &info.parent_ids)?
                .iter()
                .filter(|change| change.path.starts_with(&query.prefix))
                .cloned()
                .collect();
            if changes.is_empty() {
                continue;
            }
            entries.push(LogEntry {
                revision: objects::read_revision(&repo, info.id)?,
                changes,
            });
        }
        Ok(entries)
    }
}

impl Backend for GitBackend {
    fn head(&self) -> Result<Arc<dyn Snapshot>, BackendError> {
        let tip = self.inner.tip()?;
        Ok(self.inner.snapshot(tip)?)
    }

    fn at(&self, revision: &RevisionId) -> Result<Arc<dyn Snapshot>, BackendError> {
        let id = object_id(revision)?;
        let repo = self.inner.local();
        if repo.find_commit(id).is_err() {
            return Err(BackendError::UnknownRevision(revision.clone()));
        }
        Ok(self.inner.snapshot_at(id)?)
    }

    fn read_at(&self, revision: &RevisionId, path: &str) -> Result<Option<Vec<u8>>, BackendError> {
        let id = object_id(revision)?;
        let repo = self.inner.local();
        let Ok(commit) = repo.find_commit(id) else {
            return Err(BackendError::UnknownRevision(revision.clone()));
        };
        let tree = commit.tree_id().map_err(storage)?.detach();
        let Some(blob) = objects::blob_at(&repo, tree, path)? else {
            return Ok(None);
        };
        Ok(Some(repo.find_blob(blob).map_err(storage)?.data.clone()))
    }

    fn commit(&self, author: &Actor, write: Write<'_>) -> Result<Option<Committed>, BackendError> {
        self.inner.commit(author, write)
    }

    fn log(&self, query: &LogQuery) -> Result<Vec<LogEntry>, BackendError> {
        self.inner.log(query)
    }

    fn changes(
        &self,
        from: Option<&RevisionId>,
        to: &RevisionId,
    ) -> Result<Vec<Change>, BackendError> {
        let from = from.map(object_id).transpose()?;
        let to = object_id(to)?;
        self.inner.changes_between(from, Some(to))
    }

    fn refresh(&self) -> Result<Option<HeadMoved>, BackendError> {
        self.inner.refresh()
    }

    fn remote(&self) -> Option<&dyn Remote> {
        self.inner.remote.as_ref().map(|_| self as &dyn Remote)
    }

    fn subscribe(&self) -> broadcast::Receiver<BackendEvent> {
        self.inner.events.subscribe()
    }
}

impl Remote for GitBackend {
    fn sync(&self) -> Result<op_backend::SyncReport, BackendError> {
        sync::run(&self.inner)
    }

    fn status(&self) -> SyncStatus {
        lock(&self.inner.status).clone()
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().expect("git backend mutex poisoned")
}
