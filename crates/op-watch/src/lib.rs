use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use notify::{RecommendedWatcher, RecursiveMode, Watcher as _};
use op_git::{GitError, Repo, Worktree};
use op_store::{CONFIG_FILE, Store, StoreError};

// What a settled pass found. Tasks are named by the number their file carries — the watcher reads
// file names and git trees, so it stays on the file side of the id and leaves rendering the
// key to whoever publishes the change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    Task { number: u64, branch: String },
    // Coarse on purpose: every registry edit sends a reader back to the whole registry, so the
    // watcher names the branch and leaves the tags undiffed.
    Tags { branch: String },
    Config,
}

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
    // `store` is the one the daemon serves: its `.plan/config.toml` is the store's single
    // abbreviation, whatever another branch's copy of the file says.
    pub fn start(repo: Repo, store: Store, sink: Sender<Change>) -> Result<Self, WatchError> {
        let (tx, rx) = mpsc::channel();
        let fs_tx = tx.clone();
        let mut notifier =
            notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
                // Only ref/HEAD/worktree and .plan paths signal a re-diff; git's own index and log
                // churn (which the .git watch also sees) would otherwise trigger pointless passes.
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
        reconcile(&repo, &store, &worktrees, &mut notifier, &mut watched);
        let baseline = observe(&repo, &store, &worktrees)?;

        let worker =
            std::thread::spawn(move || run(repo, store, notifier, rx, sink, watched, baseline));
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
    store: Store,
    mut notifier: RecommendedWatcher,
    rx: Receiver<Msg>,
    sink: Sender<Change>,
    mut watched: HashSet<PathBuf>,
    mut state: State,
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
            None => {
                match attempt_pass(
                    &repo,
                    &store,
                    &mut notifier,
                    &mut watched,
                    &mut state,
                    &sink,
                ) {
                    Pass::Settled => deadline = None,
                    Pass::Retry(at) => deadline = Some(at),
                    Pass::RepoGone => {
                        tracing::warn!("git dir is gone; stopping the watcher");
                        break;
                    }
                }
            }
        }
    }
}

enum Pass {
    Settled,
    Retry(Instant),
    RepoGone,
}

// One settled diff pass. `Settled` once a clean snapshot has been taken (wait for the next fs
// event); a `SETTLE` retry when a git op is mid-flight or a read failed — never diffing a torn or
// partially-read tree.
fn attempt_pass(
    repo: &Repo,
    store: &Store,
    notifier: &mut RecommendedWatcher,
    watched: &mut HashSet<PathBuf>,
    state: &mut State,
    sink: &Sender<Change>,
) -> Pass {
    // `git worktree remove` can prune this repo's own git dir out from under it, and gix then
    // resolves nothing rather than failing: worktrees and branches read as empty, so a diff would
    // report every task on every branch as deleted. Nothing is watchable from here again either.
    if canonical_dir(&repo.git_common_dir()).is_none() {
        return Pass::RepoGone;
    }
    let Ok(worktrees) = repo.worktrees() else {
        return Pass::Retry(Instant::now() + SETTLE);
    };
    // A git rewrite leaves the tree torn and refs churning mid-op; defer until every worktree settles.
    if worktrees.iter().any(|worktree| worktree.op_in_progress) {
        return Pass::Retry(Instant::now() + SETTLE);
    }
    reconcile(repo, store, &worktrees, notifier, watched);
    match observe(repo, store, &worktrees) {
        Ok(next) => {
            emit_diff(state, &next, sink);
            *state = next;
            Pass::Settled
        }
        // A transient read error must not be read as "everything deleted"; keep the prior state and
        // retry, so the diff never emits a spurious deletion flood.
        Err(_) => Pass::Retry(Instant::now() + SETTLE),
    }
}

struct State {
    tasks: Snapshot,
    tags: TagSnapshot,
    // Compared verbatim rather than parsed: an edit that leaves the file invalid must register too,
    // so the daemon can stop rather than keep serving keys the store no longer claims.
    config: Option<String>,
}

fn observe(repo: &Repo, store: &Store, worktrees: &[Worktree]) -> Result<State, WatchError> {
    Ok(State {
        tasks: task_snapshot(repo, store, worktrees)?,
        tags: tag_snapshot(repo, store, worktrees)?,
        config: config_text(store),
    })
}

fn config_text(store: &Store) -> Option<String> {
    std::fs::read_to_string(store.plan_dir().join(CONFIG_FILE)).ok()
}

type Snapshot = HashMap<String, HashMap<u64, Cell>>;

// Only branches a live worktree holds: a tag is readable and writable through its worktree's
// registry alone, so a branch with no worktree has no registry to report on.
type TagSnapshot = HashMap<String, HashMap<String, String>>;

// A task's effective state on one branch: the working-tree blob when that branch is checked out in a
// live worktree, else its committed blob. `dirty` marks a working copy diverging from the commit;
// `deleted` a task the commit still carries but the working tree dropped.
#[derive(Clone, PartialEq, Eq)]
struct Cell {
    oid: String,
    dirty: bool,
    deleted: bool,
}

fn live_worktrees(worktrees: &[Worktree]) -> HashMap<&str, &Path> {
    let mut live: HashMap<&str, &Path> = HashMap::new();
    for worktree in worktrees {
        if worktree.op_in_progress {
            continue;
        }
        if let Some(branch) = &worktree.branch {
            live.entry(branch).or_insert(worktree.path.as_path());
        }
    }
    live
}

fn tag_snapshot(
    repo: &Repo,
    store: &Store,
    worktrees: &[Worktree],
) -> Result<TagSnapshot, WatchError> {
    let mut out = TagSnapshot::new();
    for (branch, path) in live_worktrees(worktrees) {
        let store = Store::open(path, store.abbreviation())?;
        let mut oids = HashMap::new();
        for (name, text) in store.read_all_raw_tags()? {
            oids.insert(name, repo.hash_blob(text.as_bytes())?);
        }
        out.insert(branch.to_owned(), oids);
    }
    Ok(out)
}

fn task_snapshot(
    repo: &Repo,
    store: &Store,
    worktrees: &[Worktree],
) -> Result<Snapshot, WatchError> {
    let live = live_worktrees(worktrees);

    let mut out = Snapshot::new();
    for branch in repo.local_branches()? {
        let committed = numbered(repo.branch_task_blobs(&branch)?.into_iter().collect());
        let cells = match live.get(branch.as_str()) {
            Some(path) => working_cells(&working_task_oids(repo, store, path)?, &committed),
            None => committed_cells(&committed),
        };
        out.insert(branch, cells);
    }
    Ok(out)
}

// A tree lists whatever `.plan/tasks/*.md` a commit holds; only a file a number names is a task.
fn numbered(committed: HashMap<String, String>) -> HashMap<u64, String> {
    committed
        .into_iter()
        .filter_map(|(id, oid)| Some((op_task::parse_id(&id)?, oid)))
        .collect()
}

fn committed_cells(committed: &HashMap<u64, String>) -> HashMap<u64, Cell> {
    committed
        .iter()
        .map(|(number, oid)| {
            (
                *number,
                Cell {
                    oid: oid.clone(),
                    dirty: false,
                    deleted: false,
                },
            )
        })
        .collect()
}

fn working_cells(
    working: &HashMap<u64, String>,
    committed: &HashMap<u64, String>,
) -> HashMap<u64, Cell> {
    let mut cells = HashMap::new();
    for (number, oid) in working {
        cells.insert(
            *number,
            Cell {
                oid: oid.clone(),
                dirty: committed.get(number) != Some(oid),
                deleted: false,
            },
        );
    }
    // A task the branch still commits but the working tree dropped is a live (dirty) deletion; keep
    // it so both the delete and any later restore surface as changes.
    for (number, oid) in committed {
        if !working.contains_key(number) {
            cells.insert(
                *number,
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

fn working_task_oids(
    repo: &Repo,
    store: &Store,
    worktree: &Path,
) -> Result<HashMap<u64, String>, WatchError> {
    let store = Store::open(worktree, store.abbreviation())?;
    let mut out = HashMap::new();
    for number in store.task_ids()? {
        let oid = repo.hash_blob(store.read_raw(number)?.as_bytes())?;
        out.insert(number, oid);
    }
    Ok(out)
}

fn emit_diff(old: &State, new: &State, sink: &Sender<Change>) {
    if old.config != new.config {
        let _ = sink.send(Change::Config);
    }
    for branch in old
        .tags
        .keys()
        .chain(new.tags.keys())
        .collect::<HashSet<_>>()
    {
        if old.tags.get(branch) != new.tags.get(branch) {
            let _ = sink.send(Change::Tags {
                branch: branch.clone(),
            });
        }
    }
    let empty = HashMap::new();
    let branches: HashSet<&String> = old.tasks.keys().chain(new.tasks.keys()).collect();
    for branch in branches {
        let before = old.tasks.get(branch).unwrap_or(&empty);
        let after = new.tasks.get(branch).unwrap_or(&empty);
        let numbers: HashSet<&u64> = before.keys().chain(after.keys()).collect();
        for number in numbers {
            if before.get(number) != after.get(number) {
                let _ = sink.send(Change::Task {
                    number: *number,
                    branch: branch.clone(),
                });
            }
        }
    }
}

fn reconcile(
    repo: &Repo,
    store: &Store,
    worktrees: &[Worktree],
    notifier: &mut RecommendedWatcher,
    watched: &mut HashSet<PathBuf>,
) {
    let desired = watch_paths(repo, store, worktrees);
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
            // Not a warning: a path that stays unwatchable is retried on every settled pass, so a
            // persistent failure would log on every git operation for the daemon's lifetime.
            Err(err) => tracing::debug!(path = %path.display(), %err, "watch failed"),
        }
    }
}

// The change sources: every live worktree's `.plan/tasks` and `.plan/tags` for working edits, the
// served store's own `.plan` for its `config.toml`, and the git-side refs/HEAD/worktrees under the
// shared `.git`. The common dir and `.plan` are watched non-recursively so HEAD, `packed-refs`, the
// first creation of `worktrees/` or `tags/`, and the config file register without pulling in
// objects/ or every task; the callback's `is_relevant` filter drops the index/log churn that watch
// still surfaces.
pub fn watch_paths(
    repo: &Repo,
    store: &Store,
    worktrees: &[Worktree],
) -> Vec<(PathBuf, RecursiveMode)> {
    let common = repo.git_common_dir();
    let mut paths = vec![(common.clone(), RecursiveMode::NonRecursive)];
    for sub in ["refs", "worktrees"] {
        paths.push((common.join(sub), RecursiveMode::Recursive));
    }
    let plan = store.plan_dir();
    if plan.is_dir() {
        paths.push((plan, RecursiveMode::NonRecursive));
    }
    for worktree in worktrees {
        let plan = worktree.path.join(".plan");
        // Non-recursive on `.plan` itself so the *creation* of `tags/` registers: a worktree that
        // registered no tag yet has no `tags` directory to watch, and its first tag write would
        // otherwise land unseen.
        paths.push((plan.clone(), RecursiveMode::NonRecursive));
        for sub in ["tasks", "tags"] {
            paths.push((plan.join(sub), RecursiveMode::Recursive));
        }
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

// A watched path is worth a diff pass only when it touches a task file (`.plan/tasks/*`), a tag
// file (`.plan/tags/*`), the store's `.plan/config.toml`, or a git ref source (`refs/*`, `HEAD`,
// `packed-refs`, `worktrees/*`). Everything else under `.git` — `index`, `ORIG_HEAD`,
// `COMMIT_EDITMSG`, logs — churns on routine git commands without changing any task.
fn is_relevant(event: &notify::Event) -> bool {
    event.paths.iter().any(|path| {
        if matches!(
            path.file_name().and_then(|name| name.to_str()),
            Some("HEAD" | "packed-refs")
        ) {
            return true;
        }
        let config = path.file_name().and_then(|name| name.to_str()) == Some(CONFIG_FILE);
        let mut refs = false;
        let mut worktrees = false;
        let mut plan = false;
        let mut tasks = false;
        let mut tags = false;
        for part in path.components().filter_map(|c| c.as_os_str().to_str()) {
            match part {
                "refs" => refs = true,
                "worktrees" => worktrees = true,
                ".plan" => plan = true,
                "tasks" => tasks = true,
                "tags" => tags = true,
                _ => {}
            }
        }
        refs || worktrees || (plan && (tasks || tags || config))
    })
}
