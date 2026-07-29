use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use notify::{RecommendedWatcher, RecursiveMode, Watcher as _};
use op_api::ChangeEvent;
use op_git::{GitError, Repo, Worktree};
use op_store::{Store, StoreError};

// A quiet window that coalesces the burst of fs events a single git op (commit, merge, checkout)
// produces into one diff pass.
const DEBOUNCE: Duration = Duration::from_millis(150);
// A slower cadence used while a git op is still in progress, or after a transient read error, so a
// long-running (or abandoned) op is re-checked without a tight poll.
const SETTLE: Duration = Duration::from_millis(400);

#[derive(Debug, thiserror::Error)]
pub enum WatchError {
    #[error("watch error: {0}")]
    Notify(#[from] notify::Error),
    #[error(transparent)]
    Git(#[from] GitError),
    #[error(transparent)]
    Store(#[from] StoreError),
}

pub struct Watcher {
    stop: Sender<Msg>,
    worker: Option<JoinHandle<()>>,
}

enum Msg {
    FsEvent,
    Stop,
}

impl Watcher {
    pub fn start(repo: Repo, sink: Sender<ChangeEvent>) -> Result<Self, WatchError> {
        let (tx, rx) = mpsc::channel();
        let fs_tx = tx.clone();
        let mut notifier =
            notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
                // Only ref/HEAD/worktree and .plan/tasks paths signal a re-diff; git's own index and
                // log churn (which the .git watch also sees) would otherwise trigger pointless passes.
                if let Ok(event) = res
                    && !matches!(event.kind, notify::EventKind::Access(_))
                    && is_relevant(&event)
                {
                    let _ = fs_tx.send(Msg::FsEvent);
                }
            })?;

        // Establish the watch set and baseline before returning so the first change diffs against the
        // real tree, not an empty snapshot (which would replay every task). Errors here disable the
        // watcher outright rather than seed a bogus baseline; the same reads just succeeded for the
        // daemon's initial index build, so this only fires on a genuinely broken repo.
        let worktrees = repo.worktrees()?;
        let mut watched = HashSet::new();
        reconcile(&repo, &worktrees, &mut notifier, &mut watched);
        let baseline = snapshot(&repo, &worktrees)?;

        let worker = std::thread::spawn(move || run(repo, notifier, rx, sink, watched, baseline));
        Ok(Self {
            stop: tx,
            worker: Some(worker),
        })
    }

    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        let _ = self.stop.send(Msg::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn run(
    repo: Repo,
    mut notifier: RecommendedWatcher,
    rx: Receiver<Msg>,
    sink: Sender<ChangeEvent>,
    mut watched: HashSet<PathBuf>,
    mut state: Snapshot,
) {
    let mut deadline: Option<Instant> = None;
    loop {
        let message = match deadline {
            Some(at) => match rx.recv_timeout(at.saturating_duration_since(Instant::now())) {
                Ok(message) => Some(message),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => break,
            },
            None => match rx.recv() {
                Ok(message) => Some(message),
                Err(_) => break,
            },
        };
        match message {
            Some(Msg::Stop) => break,
            Some(Msg::FsEvent) => deadline = Some(Instant::now() + DEBOUNCE),
            None => deadline = attempt_pass(&repo, &mut notifier, &mut watched, &mut state, &sink),
        }
    }
}

// One settled diff pass. Returns the next deadline: `None` once a clean snapshot has been taken (wait
// for the next fs event), or a `SETTLE` retry when a git op is mid-flight or a read failed — never
// diffing a torn or partially-read tree.
fn attempt_pass(
    repo: &Repo,
    notifier: &mut RecommendedWatcher,
    watched: &mut HashSet<PathBuf>,
    state: &mut Snapshot,
    sink: &Sender<ChangeEvent>,
) -> Option<Instant> {
    let Ok(worktrees) = repo.worktrees() else {
        return Some(Instant::now() + SETTLE);
    };
    // A git rewrite leaves the tree torn and refs churning mid-op; defer until every worktree settles.
    if worktrees.iter().any(|worktree| worktree.op_in_progress) {
        return Some(Instant::now() + SETTLE);
    }
    reconcile(repo, &worktrees, notifier, watched);
    match snapshot(repo, &worktrees) {
        Ok(next) => {
            emit_diff(state, &next, sink);
            *state = next;
            None
        }
        // A transient read error must not be read as "everything deleted"; keep the prior state and
        // retry, so the diff never emits a spurious deletion flood.
        Err(_) => Some(Instant::now() + SETTLE),
    }
}

type Snapshot = HashMap<String, HashMap<String, Cell>>;

// A task's effective state on one branch: the working-tree blob when that branch is checked out in a
// live worktree, else its committed blob. `dirty` marks a working copy diverging from the commit;
// `deleted` a task the commit still carries but the working tree dropped.
#[derive(Clone, PartialEq, Eq)]
struct Cell {
    oid: String,
    dirty: bool,
    deleted: bool,
}

fn snapshot(repo: &Repo, worktrees: &[Worktree]) -> Result<Snapshot, WatchError> {
    let mut live: HashMap<&str, &Path> = HashMap::new();
    for worktree in worktrees {
        if worktree.op_in_progress {
            continue;
        }
        if let Some(branch) = &worktree.branch {
            live.entry(branch).or_insert(worktree.path.as_path());
        }
    }

    let mut out = Snapshot::new();
    for branch in repo.local_branches()? {
        let committed: HashMap<String, String> =
            repo.branch_task_blobs(&branch)?.into_iter().collect();
        let cells = match live.get(branch.as_str()) {
            Some(path) => working_cells(&working_task_oids(repo, path)?, &committed),
            None => committed_cells(committed),
        };
        out.insert(branch, cells);
    }
    Ok(out)
}

fn committed_cells(committed: HashMap<String, String>) -> HashMap<String, Cell> {
    committed
        .into_iter()
        .map(|(id, oid)| {
            (
                id,
                Cell {
                    oid,
                    dirty: false,
                    deleted: false,
                },
            )
        })
        .collect()
}

fn working_cells(
    working: &HashMap<String, String>,
    committed: &HashMap<String, String>,
) -> HashMap<String, Cell> {
    let mut cells = HashMap::new();
    for (id, oid) in working {
        cells.insert(
            id.clone(),
            Cell {
                oid: oid.clone(),
                dirty: committed.get(id) != Some(oid),
                deleted: false,
            },
        );
    }
    // A task the branch still commits but the working tree dropped is a live (dirty) deletion; keep
    // it so both the delete and any later restore surface as changes.
    for (id, oid) in committed {
        if !working.contains_key(id) {
            cells.insert(
                id.clone(),
                Cell {
                    oid: oid.clone(),
                    dirty: true,
                    deleted: true,
                },
            );
        }
    }
    cells
}

fn working_task_oids(repo: &Repo, worktree: &Path) -> Result<HashMap<String, String>, WatchError> {
    let store = Store::open(worktree)?;
    let mut out = HashMap::new();
    for id in store.task_ids()? {
        let oid = repo.hash_blob(store.read_raw(&id)?.as_bytes())?;
        out.insert(id, oid);
    }
    Ok(out)
}

fn emit_diff(old: &Snapshot, new: &Snapshot, sink: &Sender<ChangeEvent>) {
    let empty = HashMap::new();
    let branches: HashSet<&String> = old.keys().chain(new.keys()).collect();
    for branch in branches {
        let before = old.get(branch).unwrap_or(&empty);
        let after = new.get(branch).unwrap_or(&empty);
        let ids: HashSet<&String> = before.keys().chain(after.keys()).collect();
        for id in ids {
            if before.get(id) != after.get(id) {
                let _ = sink.send(ChangeEvent::TaskChanged {
                    id: id.clone(),
                    branch: branch.clone(),
                });
            }
        }
    }
}

fn reconcile(
    repo: &Repo,
    worktrees: &[Worktree],
    notifier: &mut RecommendedWatcher,
    watched: &mut HashSet<PathBuf>,
) {
    let desired = watch_paths(repo, worktrees);
    let keep: HashSet<&PathBuf> = desired.iter().map(|(path, _)| path).collect();
    for stale in watched
        .iter()
        .filter(|path| !keep.contains(path))
        .cloned()
        .collect::<Vec<_>>()
    {
        tracing::debug!(path = %stale.display(), "unwatching");
        if let Err(err) = notifier.unwatch(&stale) {
            tracing::debug!(path = %stale.display(), %err, "unwatch failed");
        }
        watched.remove(&stale);
    }
    for (path, mode) in desired {
        if watched.contains(&path) {
            continue;
        }
        tracing::debug!(path = %path.display(), ?mode, "watching");
        match notifier.watch(&path, mode) {
            Ok(()) => {
                watched.insert(path);
            }
            Err(err) => tracing::warn!(path = %path.display(), %err, "watch failed"),
        }
    }
}

// The change sources of SPEC §7.5: every live worktree's `.plan/tasks` for working edits, and the
// git-side refs/HEAD/worktrees under the shared `.git`. The common dir is watched non-recursively so
// HEAD, `packed-refs`, and the first creation of `worktrees/` register without pulling in objects/;
// the callback's `is_relevant` filter drops the index/log churn that watch still surfaces.
pub fn watch_paths(repo: &Repo, worktrees: &[Worktree]) -> Vec<(PathBuf, RecursiveMode)> {
    let common = repo.git_common_dir();
    let mut paths = vec![(common.clone(), RecursiveMode::NonRecursive)];
    for sub in ["refs", "worktrees"] {
        paths.push((common.join(sub), RecursiveMode::Recursive));
    }
    for worktree in worktrees {
        paths.push((
            worktree.path.join(".plan").join("tasks"),
            RecursiveMode::Recursive,
        ));
    }
    paths
        .into_iter()
        .filter_map(|(path, mode)| Some((canonical_dir(&path)?, mode)))
        .collect()
}

// Only ever hand notify a canonical path. gix reports a linked worktree's common dir unnormalized,
// as `<git-dir>/../..`, and the fsevent backend resolves a path that has gone missing by deleting
// its last component until what remains exists — which on a trailing `..` appends another `../`
// instead, spinning forever on a path that can never resolve.
fn canonical_dir(path: &Path) -> Option<PathBuf> {
    let canonical = path.canonicalize().ok()?;
    canonical.is_dir().then_some(canonical)
}

// A watched path is worth a diff pass only when it touches a task file (`.plan/tasks/*`) or a git ref
// source (`refs/*`, `HEAD`, `packed-refs`, `worktrees/*`). Everything else under `.git` — `index`,
// `ORIG_HEAD`, `COMMIT_EDITMSG`, logs — churns on routine git commands without changing any task.
fn is_relevant(event: &notify::Event) -> bool {
    event.paths.iter().any(|path| {
        if matches!(
            path.file_name().and_then(|name| name.to_str()),
            Some("HEAD" | "packed-refs")
        ) {
            return true;
        }
        let mut refs = false;
        let mut worktrees = false;
        let mut plan = false;
        let mut tasks = false;
        for part in path.components().filter_map(|c| c.as_os_str().to_str()) {
            match part {
                "refs" => refs = true,
                "worktrees" => worktrees = true,
                ".plan" => plan = true,
                "tasks" => tasks = true,
                _ => {}
            }
        }
        refs || worktrees || (plan && tasks)
    })
}
