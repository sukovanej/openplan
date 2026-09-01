use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use op_task::{Abbreviation, Status, Task, Timestamp};

use crate::field::FieldUpdate;
use crate::keys::{KeyError, body_from_keys, reference_of};

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
    // The whole set replaces the old one, which is what the store validates: a name the branch does
    // not register fails the write even when the task already carried it.
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
