use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use op_task::{Abbreviation, Status, Task, Timestamp};

use crate::field::FieldUpdate;
use crate::keys::{KeyError, body_from_keys, body_from_keys_keeping, reference_of};

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

impl CreateTask {
    pub fn into_task(
        self,
        created: Timestamp,
        abbreviation: Abbreviation,
    ) -> Result<Task, KeyError> {
        let mut task = Task::new(&self.title, self.status.unwrap_or(Status::Backlog), created);
        task.set_parent(
            self.parent
                .as_deref()
                .map(|parent| reference_of(abbreviation, parent))
                .transpose()?,
        );
        task.set_dependencies(
            self.dependencies
                .iter()
                .map(|dependency| reference_of(abbreviation, dependency))
                .collect::<Result<_, _>>()?,
        );
        task.set_tags(self.tags);
        if let Some(body) = &self.body {
            task.append_body(&body_from_keys(abbreviation, body)?);
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
}

impl TaskPatch {
    pub fn apply(self, task: &mut Task, abbreviation: Abbreviation) -> Result<(), KeyError> {
        if let Some(status) = self.status {
            task.set_status(status);
        }
        match self.parent {
            FieldUpdate::Keep => {}
            FieldUpdate::Clear => task.set_parent(None),
            FieldUpdate::Set(key) => task.set_parent(Some(reference_of(abbreviation, &key)?)),
        }
        if let Some(rank) = self.rank {
            task.set_rank(Some(rank));
        }
        if let Some(dependencies) = self.dependencies {
            task.set_dependencies(
                dependencies
                    .iter()
                    .map(|dependency| reference_of(abbreviation, dependency))
                    .collect::<Result<_, _>>()?,
            );
        }
        if let Some(tags) = self.tags {
            task.set_tags(tags);
        }
        Ok(())
    }
}

// One conflict block of a task's body, exactly as `TaskDetail::body` carries it, and the text to put
// in its place: one version, both, or new text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ResolveConflict {
    pub block: String,
    pub text: String,
}

// A task's body as the web editor writes it: `base` is `TaskDetail::body` exactly as the editor last
// read it, and `text` is the new body in the same spelling. The daemon merges `text` with what other
// writers changed since `base`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct WriteBody {
    pub base: String,
    pub text: String,
}

impl WriteBody {
    // A file can hold a reference in a spelling a write may not add, such as another store's key.
    // The text may keep each one that `base` holds, so an edit elsewhere is not refused for it.
    pub fn into_bodies(self, abbreviation: Abbreviation) -> Result<(String, String), KeyError> {
        let held: Vec<&str> = op_task::body_ref_spans(&self.base)
            .into_iter()
            .map(|(_, inner)| op_task::ref_target(inner))
            .collect();
        let base = body_from_keys_keeping(abbreviation, &self.base, |_| true)?;
        let text =
            body_from_keys_keeping(abbreviation, &self.text, |target| held.contains(&target))?;
        Ok((base, text))
    }
}

// A whole task file, as `openplan get` prints one, to write back over the task. The comment log is
// append-only, so the text must keep every entry the task already has.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct WriteTaskFile {
    pub text: String,
}
