use std::path::Path;

use anyhow::{Context as _, Result, bail};
use op_api::{CreateTask, TaskDetail, TaskPatch};
use op_client::Client;
use op_git::Repo;
use op_server::serve_root;

use crate::daemon::{daemon_base_url, project_named};

// Writes go through the machine daemon, the single in-band writer: it allocates ids from one counter
// with a view across every local branch, and resolves the target worktree from the branch at write
// time. This carries the branch the caller is on, so a checkout underneath us can only ever refuse
// the write, never redirect it to another branch. It carries the project too, because the one daemon
// serves every repository on the machine and a branch name means nothing without one.
pub struct Writer {
    client: Client,
    base_url: String,
    project: String,
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
        let base_url = daemon_base_url(&client, daemon_url)?;
        let project = resolve_project(&client, &base_url, &repo, root, daemon_url.is_none())?;

        Ok(Self {
            client,
            base_url,
            project,
            branch,
        })
    }

    pub fn create(&self, task: &CreateTask) -> Result<String> {
        Ok(self
            .client
            .create_task(&self.base_url, &self.project, &self.branch, task)?)
    }

    pub fn patch(&self, id: &str, patch: &TaskPatch) -> Result<TaskDetail> {
        Ok(self
            .client
            .patch_task(&self.base_url, &self.project, &self.branch, id, patch)?)
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        Ok(self
            .client
            .delete_task(&self.base_url, &self.project, &self.branch, id)?)
    }
}

// The repository the caller stands in, as the daemon names it. A repository the machine daemon does
// not yet serve is registered here, so the first write from a fresh checkout needs no setup step.
// The POST is idempotent by repository, so two concurrent first writes both land and only one of
// them reports a registration.
//
// `may_register` is false when the caller named a daemon with `--daemon`. Registering there would
// leave a repository indexed and watched by a daemon the caller only borrowed for one command, and
// two daemons writing one checkout is exactly what the single-writer rule exists to prevent.
fn resolve_project(
    client: &Client,
    base_url: &str,
    repo: &Repo,
    root: &Path,
    may_register: bool,
) -> Result<String> {
    // Only a daemon that answered with something this client cannot read is an out-of-date one; a
    // daemon that is merely busy or unreachable says so itself, and telling that user to restart it
    // would send them after the wrong thing.
    let views = client
        .projects(base_url, op_client::WRITE_TIMEOUT)
        .map_err(|err| match err {
            op_client::ClientError::Unreadable { .. } => anyhow::anyhow!(
                "the oplan daemon at {base_url} does not serve the project routes; it predates \
                 them. Stop it (`oplan server stop`) and rerun here."
            ),
            other => anyhow::Error::new(other).context(format!(
                "the oplan daemon at {base_url} did not list its projects"
            )),
        })?;
    let mine = repo.git_common_dir();
    if let Some(name) = project_named(views, &mine) {
        return Ok(name);
    }
    if !may_register {
        bail!(
            "the oplan daemon at {base_url} does not serve {}; register it there first with \
             `oplan project add --daemon {base_url}`, or drop --daemon to use the machine daemon",
            mine.display()
        );
    }
    // The daemon's working directory is its own home, not the caller's, so a relative path would
    // resolve against the wrong directory there — and register whatever repository happens to sit
    // at that spot. `--root` defaults to `.`, so this is the ordinary case, not the exotic one.
    let serve = serve_root(repo, root);
    let serve = std::fs::canonicalize(&serve)
        .with_context(|| format!("no such directory: {}", serve.display()))?;
    let (view, created) = client.register_project(base_url, &serve)?;
    if created {
        // stderr, because stdout carries the id `oplan create` prints and scripts read.
        eprintln!("registered project {} at {}", view.name, view.root);
    }
    Ok(view.name)
}
