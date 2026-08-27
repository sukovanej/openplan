use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use op_task::{Abbreviation, Status, Task, Timestamp};

use crate::branch::BranchState;
use crate::comment::Comment;
use crate::field::{Field, Rfc3339};
use crate::metadata::{Metadata, updated_field};

// Every read surface carries the same `metadata`: the frontmatter parsed field by field, so a file
// with one bad field still renders the rest and flags only what failed. `title` and `body` come from
// the markdown, which has no schema to violate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TaskSummary {
    pub id: String,
    pub title: String,
    pub metadata: Metadata,
}

impl TaskSummary {
    pub fn from_partial(
        id: String,
        partial: op_task::PartialTask,
        abbreviation: Abbreviation,
    ) -> Self {
        Self {
            id,
            title: partial.title.unwrap_or_default(),
            metadata: Metadata::from_partial(partial.metadata, abbreviation),
        }
    }

    pub fn from_task(id: String, task: &Task, abbreviation: Abbreviation) -> Self {
        Self {
            id,
            title: task.title().unwrap_or_default(),
            metadata: Metadata::from_frontmatter(&task.frontmatter, abbreviation),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TaskView {
    pub id: String,
    pub title: String,
    pub metadata: Metadata,
    pub body: String,
    // Derived from git rather than read from the file, so it sits outside `metadata`.
    pub updated: Field<Rfc3339>,
}

impl TaskView {
    pub fn from_partial(
        id: String,
        partial: op_task::PartialTask,
        updated: op_task::FieldResult<Timestamp>,
        abbreviation: Abbreviation,
    ) -> Self {
        let metadata = Metadata::from_partial(partial.metadata, abbreviation);
        let created = metadata.created();
        Self {
            id,
            title: partial.title.unwrap_or_default(),
            updated: updated_field(created, updated),
            metadata,
            body: partial.body,
        }
    }

    // `updated` is git-derived, so only a caller holding the history can supply it; a store-only
    // read passes `None`.
    pub fn from_task(
        id: String,
        task: &Task,
        updated: op_task::FieldResult<Timestamp>,
        abbreviation: Abbreviation,
    ) -> Self {
        Self {
            id,
            title: task.title().unwrap_or_default(),
            updated: updated_field(Some(task.frontmatter.created), updated),
            metadata: Metadata::from_frontmatter(&task.frontmatter, abbreviation),
            body: task.body.clone(),
        }
    }
}

// A direct child of a task, in sibling (`rank`) order — enough to render the subtasks list without
// the whole task set in memory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TaskChild {
    pub id: String,
    pub title: String,
    pub status: Field<Status>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub rank: Option<String>,
}

// A task referenced by `[[id]]` in the body, resolved to its current title and status so a chip can
// render without the client looking it up in a full list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TaskRef {
    pub id: String,
    pub title: String,
    pub status: Field<Status>,
}

// One task read for the detail page: `metadata` parsed field by field, so a file with one bad field
// still renders everything else and flags only what failed, and `body` always the raw markdown.
// `updated` is derived from git rather than read from the file, so it sits outside `metadata`.
// Paired with every branch it lives on, so a cold-loaded page renders a branch switcher without the
// list in memory; `headline` names the branch the shown version resolves to. `parent_title`,
// `children`, and `refs` carry the immediate hierarchy so the page renders from this one read.
// `write_target` names where an edit of the shown version lands, and whether it can land there.
// `depends_on` is what this task waits for, in the order the file lists it; `blocks` is every task
// that waits for this one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TaskDetail {
    pub project: String,
    pub id: String,
    pub title: String,
    pub metadata: Metadata,
    pub body: String,
    pub updated: Field<Rfc3339>,
    pub headline: String,
    pub branches: Vec<BranchState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub write_target: Option<WriteTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub parent_title: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<TaskChild>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refs: Vec<TaskRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<TaskRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocks: Vec<TaskRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub comments: Vec<Comment>,
}

// One logical task aggregated across every branch it lives on: `metadata` and `title` come from the
// `headline` branch (the most recently changed one), plus one `branches` entry per branch so a
// client can render badges, mark the headline, and spot divergence. `project` is the coordinate the
// key alone cannot carry: two stores can commit the same abbreviation, so `id` names a task only
// within its project. `write_target` reads as it does on `TaskDetail`: a row the aggregate shows
// from another branch says here where acting on it would land.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TaskListItem {
    pub project: String,
    pub id: String,
    pub title: String,
    pub metadata: Metadata,
    // How many entries the log holds, not the log itself: a row shows a count, and shipping every
    // comment of every task would make the list read carry the whole store's prose.
    pub comment_count: usize,
    pub updated: Field<Rfc3339>,
    pub headline: String,
    pub branches: Vec<BranchState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub write_target: Option<WriteTarget>,
}

// Where a write to one task lands, and whether it can land there now. A read scoped to a branch
// writes to that branch; a read of the aggregate writes wherever the task lives. `writable` is false
// while no live worktree holds `branch` — none has it checked out, or the one that does is mid-merge
// — and the daemon then refuses every write to the task. A client reads this to offer only the
// actions that can succeed, and to name the branch that stops the rest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct WriteTarget {
    pub branch: String,
    pub writable: bool,
}

// One task a search matched, with the branch whose version of it matched. The task itself is the
// aggregated row every other list read answers with, so a hit renders exactly like a list row;
// `branch` is what the hit adds — where the matching text lives, which is the headline branch
// whenever that branch matches too.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SearchHit {
    pub task: TaskListItem,
    pub branch: String,
}

// Where a task sits. Two stores can commit the same abbreviation, so every map in the merged board
// keys on this rather than on the id alone; a parent reference then resolves in the task's own
// project, which is the only project it can name.
pub(crate) type Coordinate<'a> = (&'a str, &'a str);

pub(crate) fn coordinate(task: &TaskListItem) -> Coordinate<'_> {
    (task.project.as_str(), task.id.as_str())
}

pub(crate) fn parent_coordinate(task: &TaskListItem) -> Option<Coordinate<'_>> {
    Some((task.project.as_str(), task.metadata.parent()?))
}
