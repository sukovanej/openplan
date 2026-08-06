use std::path::Path;

use anyhow::{Context as _, Result, bail};
use op_api::{CreateTask, DaemonInfo, TaskDetail, TaskPatch};
use op_client::Client;
use op_git::Repo;
use op_server::serve_root;

use crate::daemon::{Control, base_url, default_port, same_path};

// Writes go through the machine daemon, the single in-band writer: it allocates ids from one counter
// with a view across every local branch, and resolves the target worktree from the branch at write
// time. This carries the branch the caller is on, so a checkout underneath us can only ever refuse
// the write, never redirect it to another branch.
pub struct Writer {
    client: Client,
    base_url: String,
    branch: String,
}

impl Writer {
    pub fn resolve(root: &Path, daemon_url: Option<&str>) -> Result<Self> {
        let repo = Repo::discover(root).with_context(|| {
            format!(
                "oplan writes require a git repository; none found at {}",
                root.display()
            )
        })?;
        let branch = repo.current_branch().context(
            "cannot determine the current branch (detached HEAD?); a write always targets a branch",
        )?;

        let client = Client::default();
        let (base_url, served) = match daemon_url {
            Some(url) => {
                let base = url.trim_end_matches('/').to_owned();
                let info = client
                    .health(&base)
                    .with_context(|| format!("no oplan daemon at {base}"))?;
                (base, info)
            }
            None => {
                let info = Control::resolve()?
                    .ensure_daemon(default_port(), &serve_root(&repo, root))?
                    .into_info();
                (base_url(info.port), info)
            }
        };
        ensure_same_repo(&repo, &served)?;

        Ok(Self {
            client,
            base_url,
            branch,
        })
    }

    pub fn create(&self, task: &CreateTask) -> Result<String> {
        Ok(self
            .client
            .create_task(&self.base_url, &self.branch, task)?)
    }

    pub fn patch(&self, id: &str, patch: &TaskPatch) -> Result<TaskDetail> {
        Ok(self
            .client
            .patch_task(&self.base_url, &self.branch, id, patch)?)
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        Ok(self.client.delete_task(&self.base_url, &self.branch, id)?)
    }
}

// A branch name identifies a worktree only within one repository, so a daemon serving a different
// repo must be refused rather than handed a branch name it would resolve against its own worktrees.
fn ensure_same_repo(repo: &Repo, served: &DaemonInfo) -> Result<()> {
    let mine = repo.git_common_dir();
    let Some(theirs) = served.repo.as_deref() else {
        bail!(
            "the running daemon (pid {}, port {}) predates repository identity in /health, so it \
             may be indexing another repository; restart it with `oplan server restart`",
            served.pid,
            served.port
        );
    };
    if !same_path(Path::new(theirs), &mine) {
        bail!(
            "the oplan daemon on port {} serves {theirs}, not {}; writes route through it, so stop \
             it (`oplan server stop`) and rerun here, or point this command at the right daemon \
             with `--daemon <url>`",
            served.port,
            mine.display()
        );
    }
    Ok(())
}
