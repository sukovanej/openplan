use std::path::Path;

use anyhow::{Context as _, Result, bail};
use op_api::{
    CreateTag, CreateTask, Matrix, SearchHit, TagPatch, TagView, TaskBranches, TaskDetail,
    TaskListItem, TaskPatch, TaskTreeView,
};
use op_client::Client;
use op_git::Repo;
use op_server::serve_root;

use crate::daemon::{daemon_base_url, project_named};

// The machine daemon is the store, as every task and tag command sees it: the single in-band
// writer, which allocates ids from one counter with a view across every local branch and resolves
// the target worktree from the branch at write time, and the single resolver for reads, so one
// question gets one answer whether the CLI or the web UI asked it. This carries the branch the caller is on, so a
// checkout underneath us can only ever refuse a write, never redirect it to another branch — and a
// read reports the caller's own branch rather than the serve root's. It carries the project too,
// because the one daemon serves every repository on the machine and a branch name means nothing
// without one.
//
// Reads and writes resolve together because `move` is both: it reads a sibling group and writes the
// ranks it computes from it, and those two must land on one branch of one project.
pub struct Plan {
    client: Client,
    base_url: String,
    project: String,
    branch: String,
}

impl Plan {
    pub fn resolve(root: &Path, daemon_url: Option<&str>) -> Result<Self> {
        let repo = Repo::discover(root).with_context(|| {
            format!(
                "openplan requires a git repository; none found at {}",
                root.display()
            )
        })?;
        let branch = repo.current_branch().context(
            "cannot determine the current branch (detached HEAD?); every read and write targets a \
             branch",
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

    // The branch every read and write of this command targets unless the caller named another one
    // to read.
    pub fn branch(&self) -> &str {
        &self.branch
    }

    pub fn list(&self, branch: &str) -> Result<Vec<TaskListItem>> {
        served(
            self.client
                .tasks(&self.base_url, &self.project, Some(branch)),
        )
    }

    pub fn matrix(&self) -> Result<Matrix> {
        served(self.client.matrix(&self.base_url, &self.project))
    }

    pub fn search(&self, query: &str) -> Result<Vec<SearchHit>> {
        served(self.client.search(&self.base_url, &self.project, query))
    }

    pub fn get(&self, id: &str, branch: &str) -> Result<TaskDetail> {
        served(
            self.client
                .task(&self.base_url, &self.project, id, Some(branch)),
        )
    }

    pub fn tree(&self, id: &str, branch: &str, depth: Option<usize>) -> Result<TaskTreeView> {
        served(
            self.client
                .task_tree(&self.base_url, &self.project, id, Some(branch), depth),
        )
    }

    pub fn branches(&self, id: &str) -> Result<TaskBranches> {
        served(self.client.task_branches(&self.base_url, &self.project, id))
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

    pub fn tags(&self) -> Result<Vec<TagView>> {
        served(
            self.client
                .tags(&self.base_url, &self.project, Some(&self.branch)),
        )
    }

    pub fn tag(&self, name: &str) -> Result<TagView> {
        served(
            self.client
                .tag(&self.base_url, &self.project, name, Some(&self.branch)),
        )
    }

    pub fn create_tag(&self, tag: &CreateTag) -> Result<TagView> {
        Ok(self
            .client
            .create_tag(&self.base_url, &self.project, &self.branch, tag)?)
    }

    pub fn patch_tag(&self, name: &str, patch: &TagPatch) -> Result<TagView> {
        Ok(self
            .client
            .patch_tag(&self.base_url, &self.project, &self.branch, name, patch)?)
    }

    pub fn delete_tag(&self, name: &str, force: bool) -> Result<()> {
        Ok(self
            .client
            .delete_tag(&self.base_url, &self.project, &self.branch, name, force)?)
    }
}

// A daemon older than the read routes has two ways of saying so, and neither of them says it. One
// that refuses unserved `/api/` paths answers 404 about a route, where every other 404 here is about
// a task; an older one still falls those paths through to the SPA and answers the page itself. Both
// mean the same thing: stop the daemon. JSON of the wrong shape is not one of them — that is a
// schema mismatch, which says nothing about the daemon's age and must not send anyone to stop a
// daemon other repositories are using.
fn served<T>(outcome: Result<T, op_client::ClientError>) -> Result<T> {
    let predates = |err: &op_client::ClientError| match err {
        op_client::ClientError::NotJson { .. } => true,
        op_client::ClientError::Refused { status, message } => {
            *status == 404 && message.starts_with("no such route")
        }
        _ => false,
    };
    outcome.map_err(|err| match predates(&err) {
        true => anyhow::Error::new(err).context(
            "this openplan daemon does not serve the read routes; it predates them. Stop it \
             (`openplan server stop`) and rerun here.",
        ),
        false => anyhow::Error::new(err),
    })
}

// The repository the caller stands in, as the daemon names it. A repository the machine daemon does
// not yet serve is registered here, so the first write from a fresh checkout needs no setup step.
// The POST is idempotent by repository, so two concurrent first writes both land and only one of
// them reports a registration.
//
// `may_register` is false when the caller named a daemon with `--daemon`. Registering there would
// leave a repository indexed and watched by a daemon the caller only borrowed for one command, and
// two daemons writing one checkout is exactly what the single-writer rule exists to prevent.
pub fn resolve_project(
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
                "the openplan daemon at {base_url} does not serve the project routes; it predates \
                 them. Stop it (`openplan server stop`) and rerun here."
            ),
            other => anyhow::Error::new(other).context(format!(
                "the openplan daemon at {base_url} did not list its projects"
            )),
        })?;
    let mine = repo.git_common_dir();
    if let Some(name) = project_named(views, &mine) {
        return Ok(name);
    }
    if !may_register {
        bail!(
            "the openplan daemon at {base_url} does not serve {}; register it there first with \
             `openplan project add --daemon {base_url}`, or drop --daemon to use the machine daemon",
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
        // stderr, because stdout carries the id `openplan create` prints and scripts read.
        eprintln!("registered project {} at {}", view.name, view.root);
    }
    Ok(view.name)
}
