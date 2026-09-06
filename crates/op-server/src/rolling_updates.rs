use std::sync::Arc;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

use op_api::{ChangeEvent, Conflict, Published};
use op_git::{ROLLING_UPDATES_BRANCH, Rebased, Repo};
use tokio::sync::broadcast;

use crate::project::Project;

// A burst of keystrokes becomes one commit, and a burst of commits on the default branch becomes
// one rebase. The rebase window is the longer one because a rebase costs more and nothing waits on
// it.
const COMMIT_QUIET: Duration = Duration::from_secs(5);
const REBASE_QUIET: Duration = Duration::from_secs(60);
// How often the loop looks at the default branch's tip. Reading one ref is a file read, and this
// doubles as the sweep that covers a watcher event dropped under load.
const POLL: Duration = Duration::from_secs(5);
const COMMIT_MESSAGE: &str = "Rolling task updates";
const STOPPED_REBASE: &str = "a conflict holds the rolling updates; resolve it first";
const NOTHING_TO_PUBLISH: &str =
    "the rolling-updates branch holds no task the default branch lacks";
const WORKER_GONE: &str = "the rolling-updates worker has stopped";

enum Signal {
    Edited,
    Publish(mpsc::Sender<Result<Published, String>>),
}

pub struct Handle {
    signals: mpsc::Sender<Signal>,
    repo: Repo,
}

impl Handle {
    // `None` when the repository cannot host the branch, which leaves every write where it was
    // going. A project with no default branch is the ordinary case here.
    pub fn start(project: &Arc<Project>, events: broadcast::Sender<ChangeEvent>) -> Option<Self> {
        let repo = project.repo().clone();
        let default_branch = repo.default_branch(None).ok().flatten()?;
        let driver = format!("{} merge-driver", std::env::current_exe().ok()?.display());
        if let Err(err) = repo.ensure_rolling_updates(&default_branch, &driver) {
            tracing::warn!(project = %project.name(), "rolling updates disabled: {err}");
            return None;
        }
        let (signals, inbox) = mpsc::channel();
        let worker = Worker {
            remote: repo.rolling_updates_remote(&default_branch),
            remote_branch: repo.rolling_updates_remote_branch(),
            repo: repo.clone(),
            default_branch,
            project: project.name(),
            events,
        };
        std::thread::spawn(move || worker.run(&inbox));
        Some(Handle { signals, repo })
    }

    pub fn edited(&self) {
        let _ = self.signals.send(Signal::Edited);
    }

    // Read from git every time. A person can finish the rebase in the worktree at any moment, and
    // nothing tells this process when they do, so a copy kept here would go stale and stay stale.
    pub fn conflict(&self) -> Option<Conflict> {
        self.repo
            .rolling_updates_rebase_in_progress()
            .then(|| Conflict {
                files: self.repo.rolling_updates_conflicts().unwrap_or_default(),
                worktree: self.repo.rolling_updates_worktree().display().to_string(),
            })
    }

    pub fn publish(&self) -> Result<Published, String> {
        let (reply, answer) = mpsc::channel();
        self.signals
            .send(Signal::Publish(reply))
            .map_err(|_| WORKER_GONE.to_owned())?;
        answer.recv().map_err(|_| WORKER_GONE.to_owned())?
    }
}

struct Worker {
    repo: Repo,
    default_branch: String,
    project: String,
    events: broadcast::Sender<ChangeEvent>,
    remote: String,
    remote_branch: String,
}

impl Worker {
    fn run(&self, inbox: &mpsc::Receiver<Signal>) {
        let mut tip = self.repo.branch_commit(&self.default_branch).ok();
        // Edits the CLI made while the daemon was down, and a default branch that moved meanwhile.
        let mut commit = Timer::armed(Duration::ZERO);
        let mut rebase = Timer::armed(Duration::ZERO);

        loop {
            match inbox.recv_timeout(POLL) {
                Ok(Signal::Edited) => commit.arm(COMMIT_QUIET),
                Ok(Signal::Publish(reply)) => {
                    let _ = reply.send(self.publish());
                    commit.disarm();
                }
                Err(RecvTimeoutError::Disconnected) => return,
                Err(RecvTimeoutError::Timeout) => {}
            }

            let moved = self.repo.branch_commit(&self.default_branch).ok();
            if moved != tip {
                tip = moved;
                rebase.arm(REBASE_QUIET);
            }
            if commit.fired() {
                self.commit();
            }
            if rebase.fired() {
                self.rebase();
            }
        }
    }

    fn commit(&self) {
        if self.rebase_stopped() {
            return;
        }
        match self.repo.rolling_updates_commit(COMMIT_MESSAGE) {
            Ok(true) => self.announce(),
            Ok(false) => {}
            Err(err) => self.warn(&err.to_string()),
        }
    }

    fn rebase(&self) {
        if self.rebase_stopped() {
            return;
        }
        match self.repo.rolling_updates_rebase(&self.default_branch) {
            Ok(Rebased::Clean) => self.announce(),
            Ok(Rebased::Blocked { paths }) => {
                self.warn(&format!("a conflict in {paths:?} holds the branch"));
                self.announce();
            }
            Err(err) => self.warn(&err.to_string()),
        }
    }

    fn publish(&self) -> Result<Published, String> {
        if self.rebase_stopped() {
            return Err(STOPPED_REBASE.to_owned());
        }
        if self
            .repo
            .rolling_updates_commit(COMMIT_MESSAGE)
            .map_err(text)?
        {
            self.announce();
        }
        if !self
            .repo
            .rolling_updates_differs(&self.default_branch)
            .map_err(text)?
        {
            return Err(NOTHING_TO_PUBLISH.to_owned());
        }
        let commit = self
            .repo
            .branch_commit(ROLLING_UPDATES_BRANCH)
            .map_err(text)?;
        self.repo
            .push_rolling_updates(&self.remote, &self.remote_branch)
            .map_err(text)?;
        Ok(Published {
            pull_request: crate::pull_request::open(
                &self.repo.rolling_updates_worktree(),
                self.repo.remote_url(&self.remote).as_deref(),
                &self.remote_branch,
                &self.default_branch,
            ),
            remote: self.remote.clone(),
            branch: self.remote_branch.clone(),
            commit,
        })
    }

    // A rebase that stopped owns the worktree until a person finishes it, so nothing else may
    // commit there or replay onto it.
    fn rebase_stopped(&self) -> bool {
        self.repo.rolling_updates_rebase_in_progress()
    }

    fn announce(&self) {
        let _ = self.events.send(ChangeEvent::RollingUpdatesChanged {
            project: self.project.clone(),
        });
    }

    fn warn(&self, what: &str) {
        tracing::warn!(project = %self.project, "rolling updates: {what}");
    }
}

fn text(err: op_git::GitError) -> String {
    err.to_string()
}

// One shot: it fires once and disarms itself.
struct Timer(Option<Instant>);

impl Timer {
    fn armed(delay: Duration) -> Self {
        Timer(Some(Instant::now() + delay))
    }

    fn arm(&mut self, delay: Duration) {
        *self = Timer::armed(delay);
    }

    fn disarm(&mut self) {
        self.0 = None;
    }

    fn fired(&mut self) -> bool {
        let fired = self.0.is_some_and(|at| at <= Instant::now());
        if fired {
            self.disarm();
        }
        fired
    }
}
