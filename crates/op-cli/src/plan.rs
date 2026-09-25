use std::path::Path;

use anyhow::{Context as _, Result, bail};
use op_api::{
    Comment, CreateComment, CreateTag, HistoryEntry, ProjectView, Refusal, SearchHit, SyncResult,
    SyncView, TagPatch, TagView, TaskAtRevision, TaskDetail, TaskListItem, TaskPatch, TaskTreeView,
};
use op_client::Client;
use op_server::Location;

use crate::daemon::{daemon_base_url, project_named};

// The machine daemon is the one reader and writer of every project's tasks, so every task and tag
// command goes through it. This carries the project the caller stands in.
pub struct Plan {
    client: Client,
    base_url: String,
    project: String,
}

impl Plan {
    pub fn resolve(root: &Path, daemon_url: Option<&str>) -> Result<Self> {
        let location = Location::find_or_join(root)?;
        let client = Client::default().with_identity(crate::author::identity(&location.root));
        let base_url = daemon_base_url(&client, daemon_url)?;
        let project = resolve_project(&client, &base_url, &location, daemon_url.is_none())?;
        Ok(Self {
            client,
            base_url,
            project,
        })
    }

    pub fn list(&self) -> Result<Vec<TaskListItem>> {
        served(self.client.tasks(&self.base_url, &self.project))
    }

    pub fn search(&self, query: &str) -> Result<Vec<SearchHit>> {
        served(self.client.search(&self.base_url, &self.project, query))
    }

    pub fn get(&self, id: &str) -> Result<TaskDetail> {
        served(self.client.task(&self.base_url, &self.project, id))
    }

    pub fn tree(&self, id: &str, depth: Option<usize>) -> Result<TaskTreeView> {
        served(
            self.client
                .task_tree(&self.base_url, &self.project, id, depth),
        )
    }

    pub fn comments(&self, id: &str) -> Result<Vec<Comment>> {
        served(self.client.comments(&self.base_url, &self.project, id))
    }

    pub fn comment(&self, id: &str, comment: &CreateComment) -> Result<Comment> {
        served(
            self.client
                .add_comment(&self.base_url, &self.project, id, comment),
        )
    }

    pub fn create(&self, task: &op_api::CreateTask) -> Result<String> {
        served(self.client.create_task(&self.base_url, &self.project, task))
    }

    pub fn patch(&self, id: &str, patch: &TaskPatch) -> Result<TaskDetail> {
        served(
            self.client
                .patch_task(&self.base_url, &self.project, id, patch),
        )
    }

    pub fn write(&self, id: &str, text: &str) -> Result<TaskDetail> {
        served(
            self.client
                .write_task_file(&self.base_url, &self.project, id, text),
        )
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        served(self.client.delete_task(&self.base_url, &self.project, id))
    }

    pub fn history(
        &self,
        id: Option<&str>,
        before: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<HistoryEntry>> {
        served(match id {
            Some(id) => self
                .client
                .task_history(&self.base_url, &self.project, id, before, limit),
            None => self
                .client
                .history(&self.base_url, &self.project, before, limit),
        })
    }

    pub fn revision(&self, id: &str, revision: &str) -> Result<TaskAtRevision> {
        served(
            self.client
                .task_revision(&self.base_url, &self.project, id, revision),
        )
    }

    pub fn sync(&self) -> Result<SyncResult> {
        served(self.client.sync(&self.base_url, &self.project))
    }

    pub fn sync_status(&self) -> Result<SyncView> {
        served(self.client.sync_status(&self.base_url, &self.project))
    }

    pub fn tags(&self) -> Result<Vec<TagView>> {
        served(self.client.tags(&self.base_url, &self.project))
    }

    pub fn tag(&self, name: &str) -> Result<TagView> {
        served(self.client.tag(&self.base_url, &self.project, name))
    }

    pub fn create_tag(&self, tag: &CreateTag) -> Result<TagView> {
        served(self.client.create_tag(&self.base_url, &self.project, tag))
    }

    pub fn patch_tag(&self, name: &str, patch: &TagPatch) -> Result<TagView> {
        served(
            self.client
                .patch_tag(&self.base_url, &self.project, name, patch),
        )
    }

    pub fn delete_tag(&self, name: &str, force: bool) -> Result<()> {
        served(
            self.client
                .delete_tag(&self.base_url, &self.project, name, force),
        )
    }
}

fn served<T>(outcome: Result<T, op_client::ClientError>) -> Result<T> {
    let predates = |err: &op_client::ClientError| match err {
        op_client::ClientError::NotJson { .. } => true,
        op_client::ClientError::Refused {
            status, message, ..
        } => *status == 404 && message.starts_with("no such route"),
        _ => false,
    };
    outcome.map_err(|err| {
        if let op_client::ClientError::Refused {
            reason: Some(reason),
            message,
            ..
        } = &err
        {
            return anyhow::anyhow!("{message}; {}", remedy(*reason));
        }
        match predates(&err) {
            true => anyhow::Error::new(err).context(
                "this openplan daemon does not serve these routes; it predates them. Stop it \
                 (`openplan server stop`) and rerun here.",
            ),
            false => anyhow::Error::new(err),
        }
    })
}

// The daemon states the fact; what to do about it is a spelling of this interface.
fn remedy(reason: Refusal) -> &'static str {
    match reason {
        Refusal::TagReferenced => "pass --force to delete it and leave those references dangling",
        Refusal::TagUnregistered => "register it with `openplan tag create <name>`",
    }
}

// The project the caller stands in, as the daemon names it. A project the machine daemon does not
// serve yet is registered here, so the first command from a fresh clone needs no setup step.
//
// `may_register` is false when the caller named a daemon with `--daemon`: registering there would
// leave a project served by a daemon the caller only borrowed for one command.
pub fn resolve_project(
    client: &Client,
    base_url: &str,
    location: &Location,
    may_register: bool,
) -> Result<String> {
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
    if let Some(name) = project_named(&views, location) {
        return Ok(name);
    }
    if !may_register {
        bail!(
            "the openplan daemon at {base_url} does not serve {}; register it there first with \
             `openplan project add --daemon {base_url}`, or drop --daemon to use the machine daemon",
            location.root.display()
        );
    }
    let (view, created) =
        register(client, base_url, location).context("the daemon did not take the project")?;
    if created {
        // stderr, because stdout carries the id `openplan create` prints and scripts read.
        eprintln!("registered project {} at {}", view.name, view.root);
    }
    Ok(view.name)
}

fn register(
    client: &Client,
    base_url: &str,
    location: &Location,
) -> Result<(ProjectView, bool), op_client::ClientError> {
    client.register_project(base_url, &location.root, Some(location.kind), None)
}
