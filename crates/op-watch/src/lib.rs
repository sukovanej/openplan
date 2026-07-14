use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use notify::{RecursiveMode, Watcher as _};
use op_api::ChangeEvent;
use op_git::Repo;
use op_store::Store;

// A quiet window that coalesces the burst of fs events a single git op (commit, merge, checkout)
// produces into one diff pass. While a git op is still in progress the pass is deferred and this
// same interval doubles as the settle poll.
const DEBOUNCE: Duration = Duration::from_millis(150);

#[derive(Debug, thiserror::Error)]
pub enum WatchError {
    #[error("watch error: {0}")]
    Notify(#[from] notify::Error),
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
                // Metadata/open events carry no content change; every other kind is a signal to re-diff.
                if let Ok(event) = res
                    && !matches!(event.kind, notify::EventKind::Access(_))
                {
                    let _ = fs_tx.send(Msg::FsEvent);
                }
            })?;

        // Take the watch set and baseline before returning so the first change after start() diffs
        // against the real tree rather than an empty snapshot (which would replay every task).
        let mut watched = HashSet::new();
        reconcile(&repo, &mut notifier, &mut watched);
        let baseline = snapshot(&repo);

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
    mut notifier: notify::RecommendedWatcher,
    rx: Receiver<Msg>,
    sink: Sender<ChangeEvent>,
    mut watched: HashSet<PathBuf>,
    mut state: Snapshot,
) {
    let mut pending: Option<Instant> = None;
    loop {
        let message = match pending {
            Some(_) => match rx.recv_timeout(DEBOUNCE) {
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
            Some(Msg::FsEvent) => pending = Some(Instant::now()),
            None => {
                // A git op leaves the tree torn mid-flight; wait for it to settle, re-arming the
                // debounce so the next timeout re-checks, before diffing anything.
                if repo.op_in_progress() {
                    pending = Some(Instant::now());
                    continue;
                }
                reconcile(&repo, &mut notifier, &mut watched);
                let next = snapshot(&repo);
                emit_diff(&state, &next, &sink);
                state = next;
                pending = None;
            }
        }
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

fn snapshot(repo: &Repo) -> Snapshot {
    let worktrees = repo.worktrees().unwrap_or_default();
    let mut live: HashMap<String, PathBuf> = HashMap::new();
    for worktree in &worktrees {
        if worktree.op_in_progress {
            continue;
        }
        if let Some(branch) = &worktree.branch {
            live.entry(branch.clone())
                .or_insert_with(|| worktree.path.clone());
        }
    }

    let mut out = Snapshot::new();
    for branch in repo.local_branches().unwrap_or_default() {
        let committed: HashMap<String, String> = repo
            .branch_task_blobs(&branch)
            .unwrap_or_default()
            .into_iter()
            .collect();
        let cells = match live
            .get(&branch)
            .and_then(|path| working_task_oids(repo, path))
        {
            Some(working) => working_cells(&working, &committed),
            None => committed_cells(committed),
        };
        out.insert(branch, cells);
    }
    out
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

fn working_task_oids(repo: &Repo, worktree: &Path) -> Option<HashMap<String, String>> {
    let store = Store::open(worktree).ok()?;
    let mut out = HashMap::new();
    for id in store.task_ids().ok()? {
        if let Ok(raw) = store.read_raw(&id)
            && let Ok(oid) = repo.hash_blob(raw.as_bytes())
        {
            out.insert(id, oid);
        }
    }
    Some(out)
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
    notifier: &mut notify::RecommendedWatcher,
    watched: &mut HashSet<PathBuf>,
) {
    let desired = watch_paths(repo);
    let keep: HashSet<&PathBuf> = desired.iter().map(|(path, _)| path).collect();
    for stale in watched
        .iter()
        .filter(|path| !keep.contains(path))
        .cloned()
        .collect::<Vec<_>>()
    {
        let _ = notifier.unwatch(&stale);
        watched.remove(&stale);
    }
    for (path, mode) in desired {
        if !watched.contains(&path) && notifier.watch(&path, mode).is_ok() {
            watched.insert(path);
        }
    }
}

// The change sources of SPEC §7.5: every live worktree's `.plan/tasks` for working edits, and the
// git-side refs/HEAD/worktrees under the shared `.git`. The common dir is watched non-recursively so
// HEAD, `packed-refs`, and the first creation of `worktrees/` register without pulling in objects/.
fn watch_paths(repo: &Repo) -> Vec<(PathBuf, RecursiveMode)> {
    let common = repo.git_common_dir();
    let mut paths = vec![(common.clone(), RecursiveMode::NonRecursive)];
    for sub in ["refs", "worktrees"] {
        let dir = common.join(sub);
        if dir.is_dir() {
            paths.push((dir, RecursiveMode::Recursive));
        }
    }
    for worktree in repo.worktrees().unwrap_or_default() {
        let tasks = worktree.path.join(".plan").join("tasks");
        if tasks.is_dir() {
            paths.push((tasks, RecursiveMode::Recursive));
        }
    }
    paths
}
