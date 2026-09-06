use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use op_api::{ChangeEvent, MatrixCell, Published, SyncState, SyncStatus};
use op_git::{LANE_BRANCH, Rebased, Repo};
use tokio::sync::broadcast;

use crate::project::Project;

// A burst of keystrokes becomes one commit, and a burst of trunk commits becomes one rebase. The
// rebase window is the longer one because a rebase costs more and nothing waits on it.
const COMMIT_QUIET: Duration = Duration::from_secs(5);
const REFRESH_QUIET: Duration = Duration::from_secs(60);
// How often the loop looks at the trunk tip. Reading one ref is a file read, and this doubles as
// the sweep that covers a watcher event dropped under load.
const POLL: Duration = Duration::from_secs(5);

enum Signal {
    Edited,
    Publish(mpsc::Sender<Result<Published, String>>),
}

#[derive(Default)]
struct Shared {
    state: SyncState,
    conflicted: Vec<String>,
}

pub struct Lane {
    signals: mpsc::Sender<Signal>,
    shared: Arc<Mutex<Shared>>,
    worktree: PathBuf,
}

impl Lane {
    // `None` when the repository cannot host the lane, which leaves every write on the branch it
    // would have used before. A project with no default branch is the ordinary case here.
    pub fn start(project: &Arc<Project>, events: broadcast::Sender<ChangeEvent>) -> Option<Self> {
        let repo = project.repo().clone();
        let trunk = repo.default_branch(None).ok().flatten()?;
        let driver = format!("{} merge-driver", std::env::current_exe().ok()?.display());
        let worktree = match repo.ensure_lane(&trunk, &driver) {
            Ok(worktree) => worktree,
            Err(err) => {
                tracing::warn!(project = %project.name(), "rolling updates disabled: {err}");
                return None;
            }
        };
        let (signals, inbox) = mpsc::channel();
        let shared = Arc::new(Mutex::new(Shared::default()));
        let lane = Lane {
            signals,
            shared: Arc::clone(&shared),
            worktree: worktree.clone(),
        };
        let name = project.name();
        std::thread::spawn(move || run(&repo, &trunk, &name, &shared, &inbox, &events));
        Some(lane)
    }

    pub fn worktree(&self) -> &std::path::Path {
        &self.worktree
    }

    pub fn edited(&self) {
        let _ = self.signals.send(Signal::Edited);
    }

    pub fn status(&self, pending: Vec<MatrixCell>) -> SyncStatus {
        let shared = self.shared.lock().expect("lane status poisoned");
        let state = match shared.state {
            SyncState::InSync if !pending.is_empty() => SyncState::Pending,
            other => other,
        };
        SyncStatus {
            state,
            pending,
            conflicted: shared.conflicted.clone(),
            worktree: self.worktree.display().to_string(),
        }
    }

    pub fn publish(&self) -> Result<Published, String> {
        let (reply, answer) = mpsc::channel();
        self.signals
            .send(Signal::Publish(reply))
            .map_err(|_| "the rolling-updates worker has stopped".to_owned())?;
        answer
            .recv()
            .map_err(|_| "the rolling-updates worker has stopped".to_owned())?
    }
}

fn run(
    repo: &Repo,
    trunk: &str,
    project: &str,
    shared: &Arc<Mutex<Shared>>,
    inbox: &mpsc::Receiver<Signal>,
    events: &broadcast::Sender<ChangeEvent>,
) {
    let backup = repo.config_string("openplan.backupRemote");
    let mut trunk_tip = repo.branch_commit(trunk).ok();
    // Edits the CLI made while the daemon was down, and a trunk that moved meanwhile.
    let mut commit_due = Some(Instant::now());
    let mut refresh_due = Some(Instant::now());

    loop {
        match inbox.recv_timeout(POLL) {
            Ok(Signal::Edited) => commit_due = Some(Instant::now() + COMMIT_QUIET),
            Ok(Signal::Publish(reply)) => {
                let _ = reply.send(publish(
                    repo,
                    trunk,
                    shared,
                    events,
                    project,
                    backup.as_deref(),
                ));
                commit_due = None;
            }
            Err(RecvTimeoutError::Disconnected) => return,
            Err(RecvTimeoutError::Timeout) => {}
        }

        let moved = repo.branch_commit(trunk).ok();
        if moved != trunk_tip {
            trunk_tip = moved;
            refresh_due = Some(Instant::now() + REFRESH_QUIET);
        }
        if due(&mut commit_due) {
            commit(repo, shared, events, project, backup.as_deref());
        }
        if due(&mut refresh_due) {
            refresh(repo, trunk, shared, events, project, backup.as_deref());
        }
    }
}

fn due(at: &mut Option<Instant>) -> bool {
    if at.is_some_and(|at| at <= Instant::now()) {
        *at = None;
        return true;
    }
    false
}

// A rebase that stopped owns the worktree until a person finishes it, so nothing else may commit
// there or replay onto it.
fn held(repo: &Repo, shared: &Arc<Mutex<Shared>>) -> bool {
    if !repo.lane_rebase_in_progress() {
        return false;
    }
    let mut shared = shared.lock().expect("lane status poisoned");
    shared.state = SyncState::Blocked;
    shared.conflicted = repo.lane_conflicts().unwrap_or_default();
    true
}

fn commit(
    repo: &Repo,
    shared: &Arc<Mutex<Shared>>,
    events: &broadcast::Sender<ChangeEvent>,
    project: &str,
    backup: Option<&str>,
) {
    if held(repo, shared) {
        return;
    }
    match repo.lane_commit("Ambient task edits") {
        Ok(true) => moved(repo, shared, events, project, backup),
        Ok(false) => {}
        Err(err) => tracing::warn!(project, "rolling updates: {err}"),
    }
}

fn refresh(
    repo: &Repo,
    trunk: &str,
    shared: &Arc<Mutex<Shared>>,
    events: &broadcast::Sender<ChangeEvent>,
    project: &str,
    backup: Option<&str>,
) {
    if held(repo, shared) {
        return;
    }
    set(shared, SyncState::Syncing, Vec::new());
    match repo.lane_rebase(trunk) {
        Ok(Rebased::Clean) => {
            set(shared, SyncState::InSync, Vec::new());
            moved(repo, shared, events, project, backup);
        }
        Ok(Rebased::Blocked { paths }) => {
            tracing::warn!(project, "rolling updates held by a conflict in {paths:?}");
            set(shared, SyncState::Blocked, paths);
            announce(events, project);
        }
        Err(err) => {
            tracing::warn!(project, "rolling updates: {err}");
            set(shared, SyncState::InSync, Vec::new());
        }
    }
}

fn publish(
    repo: &Repo,
    trunk: &str,
    shared: &Arc<Mutex<Shared>>,
    events: &broadcast::Sender<ChangeEvent>,
    project: &str,
    backup: Option<&str>,
) -> Result<Published, String> {
    if held(repo, shared) {
        return Err("a conflict holds the rolling updates; resolve it first".to_owned());
    }
    if let Err(err) = repo.lane_commit("Ambient task edits") {
        return Err(err.to_string());
    }
    let tip = repo.branch_commit(LANE_BRANCH).map_err(|e| e.to_string())?;
    set(shared, SyncState::Syncing, Vec::new());
    let result = repo.fast_forward(trunk, &tip).map_err(|e| e.to_string());
    set(shared, SyncState::InSync, Vec::new());
    moved(repo, shared, events, project, backup);
    result.map(|()| Published {
        branch: trunk.to_owned(),
        commit: tip,
    })
}

fn moved(
    repo: &Repo,
    _shared: &Arc<Mutex<Shared>>,
    events: &broadcast::Sender<ChangeEvent>,
    project: &str,
    backup: Option<&str>,
) {
    announce(events, project);
    // Durability only, and never on the path of an edit: a mirror nobody pulls cannot lose a race
    // this machine is the only writer of.
    if let Some(remote) = backup
        && let Err(err) = repo.push_lane(remote)
    {
        tracing::warn!(project, "rolling-updates backup push failed: {err}");
    }
}

fn set(shared: &Arc<Mutex<Shared>>, state: SyncState, conflicted: Vec<String>) {
    let mut shared = shared.lock().expect("lane status poisoned");
    shared.state = state;
    shared.conflicted = conflicted;
}

fn announce(events: &broadcast::Sender<ChangeEvent>, project: &str) {
    let _ = events.send(ChangeEvent::SyncChanged {
        project: project.to_owned(),
    });
}
