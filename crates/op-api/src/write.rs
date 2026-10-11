use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use op_forge::{Forge, PullRequest, PullRequestError};
use op_task::content::Text;
use op_task::{ProjectCode, Status, Task, Timestamp};

use crate::field::FieldUpdate;
use crate::keys::{KeyError, body_from_keys, body_from_keys_keeping, link_of};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WriteError {
    #[error(transparent)]
    Key(#[from] KeyError),
    #[error(transparent)]
    PullRequest(#[from] PullRequestError),
}

// An entry is an address, or the number of a pull request in the repository of the project.
fn pull_requests_of(
    entries: &[String],
    forge: Option<&Forge>,
) -> Result<Vec<PullRequest>, PullRequestError> {
    entries
        .iter()
        .map(|entry| PullRequest::resolve(entry, forge))
        .collect()
}

fn addresses(pull_requests: &[PullRequest]) -> Vec<String> {
    pull_requests.iter().map(PullRequest::url).collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CreateTask {
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<Status>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pull_requests: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

impl CreateTask {
    pub fn into_task(
        self,
        created: Timestamp,
        project_code: ProjectCode,
        forge: Option<&Forge>,
    ) -> Result<Task, WriteError> {
        let mut task = Task::new(&self.title, self.status.unwrap_or(Status::Backlog), created);
        task.set_parent(
            self.parent
                .as_deref()
                .map(|parent| link_of(project_code, parent))
                .transpose()?,
        );
        task.set_dependencies(
            self.dependencies
                .iter()
                .map(|dependency| link_of(project_code, dependency))
                .collect::<Result<_, KeyError>>()?,
        );
        task.set_tags(self.tags);
        task.set_pull_requests(addresses(&pull_requests_of(&self.pull_requests, forge)?));
        if let Some(body) = &self.body {
            task.append_body(&body_from_keys(project_code, body)?);
        }
        Ok(task)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TaskPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub status: Option<Status>,
    #[serde(default, skip_serializing_if = "FieldUpdate::is_keep")]
    #[schema(value_type = Option<String>)]
    pub parent: FieldUpdate<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub rank: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub dependencies: Option<Vec<String>>,
    // The whole set replaces the old one, which is what the tracker validates: a name the project
    // does not register fails the write even when the task already carried it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub tags: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub pull_requests: Option<Vec<String>>,
    // Each changes the set as the write finds it, so two writers that add at the same time both
    // keep their entry.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub add_pull_requests: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remove_pull_requests: Vec<String>,
}

impl TaskPatch {
    pub fn apply(
        self,
        task: &mut Task,
        project_code: ProjectCode,
        forge: Option<&Forge>,
    ) -> Result<(), WriteError> {
        if let Some(status) = self.status {
            task.set_status(status);
        }
        match self.parent {
            FieldUpdate::Keep => {}
            FieldUpdate::Clear => task.set_parent(None),
            FieldUpdate::Set(key) => task.set_parent(Some(link_of(project_code, &key)?)),
        }
        if let Some(rank) = self.rank {
            task.set_rank(Some(rank));
        }
        if let Some(dependencies) = self.dependencies {
            task.set_dependencies(
                dependencies
                    .iter()
                    .map(|dependency| link_of(project_code, dependency))
                    .collect::<Result<_, KeyError>>()?,
            );
        }
        if let Some(tags) = self.tags {
            task.set_tags(tags);
        }
        if let Some(pull_requests) = self.pull_requests {
            task.set_pull_requests(addresses(&pull_requests_of(&pull_requests, forge)?));
        }
        if !(self.add_pull_requests.is_empty() && self.remove_pull_requests.is_empty()) {
            let added = pull_requests_of(&self.add_pull_requests, forge)?;
            let removed = pull_requests_of(&self.remove_pull_requests, forge)?;
            // A forge compares a repository without case, so an address that differs only in case
            // names a pull request the set already holds.
            let named_by = |address: &str, named: &[PullRequest]| {
                PullRequest::parse(address)
                    .is_ok_and(|held| named.iter().any(|other| other.is(&held)))
            };
            let mut held = task.frontmatter.pull_requests.clone();
            held.retain(|address| !named_by(address, &removed) && !named_by(address, &added));
            held.extend(addresses(&added));
            task.set_pull_requests(held);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TaskText {
    pub title: String,
    pub description: String,
}

// The web editor's save: `base` is the text exactly as the editor last read it from `TaskDetail`, and
// the daemon merges `text` with what other writers changed since.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct WriteTaskText {
    pub base: TaskText,
    pub text: TaskText,
}

impl WriteTaskText {
    pub fn into_texts(self, project_code: ProjectCode) -> Result<(Text, Text), KeyError> {
        texts(
            project_code,
            (self.base.title, &self.base.description),
            (self.text.title, &self.text.description),
        )
    }
}

// A file can hold a reference in a spelling a write may not add, such as another store's key. The
// text may keep each one that `base` holds, so an edit elsewhere is not refused for it.
pub(crate) fn texts(
    project_code: ProjectCode,
    base: (String, &str),
    text: (String, &str),
) -> Result<(Text, Text), KeyError> {
    let held: Vec<&str> = op_task::body_ref_spans(base.1)
        .into_iter()
        .map(|(_, inner)| op_task::ref_target(inner))
        .collect();
    let base_description = body_from_keys_keeping(project_code, base.1, |_| true)?;
    let description =
        body_from_keys_keeping(project_code, text.1, |target| held.contains(&target))?;
    Ok((
        Text {
            title: base.0,
            description: base_description,
        },
        Text {
            title: text.0,
            description,
        },
    ))
}

// A whole task file, as `openplan tasks get` prints one, to write back over the task. The comment
// log is append-only, so the text must keep every entry the task already has.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct WriteTaskFile {
    pub text: String,
}
