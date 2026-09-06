use std::path::{Path, PathBuf};
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
const COMMIT_MESSAGE: &str = "Rolling task updates";

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
        let worker = Worker {
            backup: repo.config_string("openplan.backupRemote"),
            repo,
            default_branch,
            project: project.name(),
            shared: Arc::clone(&shared),
            events,
        };
        std::thread::spawn(move || worker.run(&inbox));
        Some(RollingUpdates {
            signals,
            shared,
            worktree,
        })
    }

    pub fn worktree(&self) -> &Path {
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

struct Worker {
    repo: Repo,
    default_branch: String,
    project: String,
    shared: Arc<Mutex<Shared>>,
    events: broadcast::Sender<ChangeEvent>,
    backup: Option<String>,
}

impl Worker {
    fn run(&self, inbox: &mpsc::Receiver<Signal>) {
        let mut tip = self.repo.branch_commit(&self.default_branch).ok();
        // Edits the CLI made while the daemon was down, and a default branch that moved meanwhile.
        let mut commit_due = Some(Instant::now());
        let mut refresh_due = Some(Instant::now());

        loop {
            match inbox.recv_timeout(POLL) {
                Ok(Signal::Edited) => commit_due = Some(Instant::now() + COMMIT_QUIET),
                Ok(Signal::Publish(reply)) => {
                    let _ = reply.send(self.publish());
                    commit_due = None;
                }
                Err(RecvTimeoutError::Disconnected) => return,
                Err(RecvTimeoutError::Timeout) => {}
            }

            let now = self.repo.branch_commit(&self.default_branch).ok();
            if now != tip {
                tip = now;
                refresh_due = Some(Instant::now() + REFRESH_QUIET);
            }
            if due(&mut commit_due) {
                self.commit();
            }
            if due(&mut refresh_due) {
                self.refresh();
            }
        }
    }

    fn commit(&self) {
        if let Some(conflict) = self.stopped_rebase() {
            self.set_conflict(Some(conflict));
            return;
        }
        match self.repo.rolling_updates_commit(COMMIT_MESSAGE) {
            Ok(true) => {
                self.announce();
                self.push_backup();
            }
            Ok(false) => {}
            Err(err) => tracing::warn!(project = %self.project, "rolling updates: {err}"),
        }
    }

    fn refresh(&self) {
        if let Some(conflict) = self.stopped_rebase() {
            self.set_conflict(Some(conflict));
            return;
        }
        match self.repo.rolling_updates_rebase(&self.default_branch) {
            Ok(Rebased::Clean) => {
                self.set_conflict(None);
                self.announce();
                self.push_backup();
            }
            Ok(Rebased::Blocked { paths }) => {
                tracing::warn!(project = %self.project, "rolling updates held by a conflict in {paths:?}");
                self.set_conflict(Some(self.conflict(paths)));
                self.announce();
            }
            Err(err) => {
                tracing::warn!(project = %self.project, "rolling updates: {err}");
                self.set_conflict(None);
            }
        }
    }

    fn publish(&self) -> Result<Published, String> {
        if let Some(conflict) = self.stopped_rebase() {
            self.set_conflict(Some(conflict));
            return Err("a conflict holds the rolling updates; resolve it first".to_owned());
        }
        self.repo
            .rolling_updates_commit(COMMIT_MESSAGE)
            .map_err(|err| err.to_string())?;
        let tip = self
            .repo
            .branch_commit(ROLLING_UPDATES_BRANCH)
            .map_err(|err| err.to_string())?;
        let fast_forward = self
            .repo
            .fast_forward(&self.default_branch, &tip)
            .map_err(|err| err.to_string());
        self.set_conflict(None);
        self.announce();
        self.push_backup();
        fast_forward.map(|()| Published {
            branch: self.default_branch.clone(),
            commit: tip,
        })
    }

    // A rebase that stopped owns the worktree until a person finishes it, so nothing else may
    // commit there or replay onto it.
    fn stopped_rebase(&self) -> Option<Conflict> {
        self.repo
            .rolling_updates_rebase_in_progress()
            .then(|| self.conflict(self.repo.rolling_updates_conflicts().unwrap_or_default()))
    }

    fn conflict(&self, files: Vec<String>) -> Conflict {
        Conflict {
            files,
            worktree: self.repo.rolling_updates_worktree().display().to_string(),
        }
    }

    fn set_conflict(&self, conflict: Option<Conflict>) {
        self.shared
            .lock()
            .expect("rolling-updates mutex poisoned")
            .conflict = conflict;
    }

    fn announce(&self) {
        let _ = self.events.send(ChangeEvent::RollingUpdatesChanged {
            project: self.project.clone(),
        });
    }

    // Durability only, and never on the path of an edit: a mirror nobody pulls cannot lose a race
    // this machine is the only writer of.
    fn push_backup(&self) {
        if let Some(remote) = &self.backup
            && let Err(err) = self.repo.push_rolling_updates(remote)
        {
            tracing::warn!(project = %self.project, "rolling-updates backup push failed: {err}");
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
