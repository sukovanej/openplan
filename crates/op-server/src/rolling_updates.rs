use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use op_api::{ChangeEvent, Conflict, MatrixCell, Published, RollingUpdates as Pending};
use op_git::{ROLLING_UPDATES_BRANCH, Rebased, Repo};
use tokio::sync::broadcast;

use crate::project::Project;

// A burst of keystrokes becomes one commit, and a burst of commits on the default branch becomes one rebase. The
// rebase window is the longer one because a rebase costs more and nothing waits on it.
const COMMIT_QUIET: Duration = Duration::from_secs(5);
const REFRESH_QUIET: Duration = Duration::from_secs(60);
// How often the loop looks at the default branch's tip. Reading one ref is a file read, and this doubles as
// the sweep that covers a watcher event dropped under load.
const POLL: Duration = Duration::from_secs(5);

enum Signal {
    Edited,
    Publish(mpsc::Sender<Result<Published, String>>),
}

#[derive(Default)]
struct Shared {
    conflict: Option<Conflict>,
}

pub struct RollingUpdates {
    signals: mpsc::Sender<Signal>,
    shared: Arc<Mutex<Shared>>,
    worktree: PathBuf,
}

impl RollingUpdates {
    // `None` when the repository cannot host the branch, which leaves every write where it was
    // going. A project with no default branch is the ordinary case here.
    pub fn start(project: &Arc<Project>, events: broadcast::Sender<ChangeEvent>) -> Option<Self> {
        let repo = project.repo().clone();
        let default_branch = repo.default_branch(None).ok().flatten()?;
        let driver = format!("{} merge-driver", std::env::current_exe().ok()?.display());
        let worktree = match repo.ensure_rolling_updates(&default_branch, &driver) {
            Ok(worktree) => worktree,
            Err(err) => {
                tracing::warn!(project = %project.name(), "rolling updates disabled: {err}");
                return None;
            }
        };
        let (signals, inbox) = mpsc::channel();
        let shared = Arc::new(Mutex::new(Shared::default()));
        let rolling = RollingUpdates {
            signals,
            shared: Arc::clone(&shared),
            worktree: worktree.clone(),
        };
        let name = project.name();
        std::thread::spawn(move || run(&repo, &default_branch, &name, &shared, &inbox, &events));
        Some(rolling)
    }

    pub fn worktree(&self) -> &std::path::Path {
        &self.worktree
    }

    pub fn edited(&self) {
        let _ = self.signals.send(Signal::Edited);
    }

    pub fn pending(&self, pending: Vec<MatrixCell>) -> Pending {
        Pending {
            pending,
            conflict: self
                .shared
                .lock()
                .expect("rolling-updates mutex poisoned")
                .conflict
                .clone(),
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
    default_branch: &str,
    project: &str,
    shared: &Arc<Mutex<Shared>>,
    inbox: &mpsc::Receiver<Signal>,
    events: &broadcast::Sender<ChangeEvent>,
) {
    let backup = repo.config_string("openplan.backupRemote");
    let mut tip = repo.branch_commit(default_branch).ok();
    // Edits the CLI made while the daemon was down, and a default branch that moved meanwhile.
    let mut commit_due = Some(Instant::now());
    let mut refresh_due = Some(Instant::now());

    loop {
        match inbox.recv_timeout(POLL) {
            Ok(Signal::Edited) => commit_due = Some(Instant::now() + COMMIT_QUIET),
            Ok(Signal::Publish(reply)) => {
                let _ = reply.send(publish(
                    repo,
                    default_branch,
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

        let moved = repo.branch_commit(default_branch).ok();
        if moved != tip {
            tip = moved;
            refresh_due = Some(Instant::now() + REFRESH_QUIET);
        }
        if due(&mut commit_due) {
            commit(repo, shared, events, project, backup.as_deref());
        }
        if due(&mut refresh_due) {
            refresh(
                repo,
                default_branch,
                shared,
                events,
                project,
                backup.as_deref(),
            );
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
fn held(repo: &Repo, shared: &Arc<Mutex<Shared>>, worktree: &std::path::Path) -> bool {
    if !repo.rolling_updates_rebase_in_progress() {
        return false;
    }
    set(shared, Some(conflict(repo, worktree)));
    true
}

fn conflict(repo: &Repo, worktree: &std::path::Path) -> Conflict {
    Conflict {
        files: repo.rolling_updates_conflicts().unwrap_or_default(),
        worktree: worktree.display().to_string(),
    }
}

fn commit(
    repo: &Repo,
    shared: &Arc<Mutex<Shared>>,
    events: &broadcast::Sender<ChangeEvent>,
    project: &str,
    backup: Option<&str>,
) {
    if held(repo, shared, &repo.rolling_updates_worktree()) {
        return;
    }
    match repo.rolling_updates_commit("Rolling task updates") {
        Ok(true) => moved(repo, shared, events, project, backup),
        Ok(false) => {}
        Err(err) => tracing::warn!(project, "rolling updates: {err}"),
    }
}

fn refresh(
    repo: &Repo,
    default_branch: &str,
    shared: &Arc<Mutex<Shared>>,
    events: &broadcast::Sender<ChangeEvent>,
    project: &str,
    backup: Option<&str>,
) {
    if held(repo, shared, &repo.rolling_updates_worktree()) {
        return;
    }
    match repo.rolling_updates_rebase(default_branch) {
        Ok(Rebased::Clean) => {
            set(shared, None);
            moved(repo, shared, events, project, backup);
        }
        Ok(Rebased::Blocked { paths }) => {
            tracing::warn!(project, "rolling updates held by a conflict in {paths:?}");
            set(
                shared,
                Some(conflict(repo, &repo.rolling_updates_worktree())),
            );
            let _ = paths;
            announce(events, project);
        }
        Err(err) => {
            tracing::warn!(project, "rolling updates: {err}");
            set(shared, None);
        }
    }
}

fn publish(
    repo: &Repo,
    default_branch: &str,
    shared: &Arc<Mutex<Shared>>,
    events: &broadcast::Sender<ChangeEvent>,
    project: &str,
    backup: Option<&str>,
) -> Result<Published, String> {
    if held(repo, shared, &repo.rolling_updates_worktree()) {
        return Err("a conflict holds the rolling updates; resolve it first".to_owned());
    }
    if let Err(err) = repo.rolling_updates_commit("Rolling task updates") {
        return Err(err.to_string());
    }
    let tip = repo
        .branch_commit(ROLLING_UPDATES_BRANCH)
        .map_err(|e| e.to_string())?;
    let result = repo
        .fast_forward(default_branch, &tip)
        .map_err(|e| e.to_string());
    set(shared, None);
    moved(repo, shared, events, project, backup);
    result.map(|()| Published {
        branch: default_branch.to_owned(),
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
        && let Err(err) = repo.push_rolling_updates(remote)
    {
        tracing::warn!(project, "rolling-updates backup push failed: {err}");
    }
}

fn set(shared: &Arc<Mutex<Shared>>, conflict: Option<Conflict>) {
    shared
        .lock()
        .expect("rolling-updates mutex poisoned")
        .conflict = conflict;
}

fn announce(events: &broadcast::Sender<ChangeEvent>, project: &str) {
    let _ = events.send(ChangeEvent::RollingUpdatesChanged {
        project: project.to_owned(),
    });
}
