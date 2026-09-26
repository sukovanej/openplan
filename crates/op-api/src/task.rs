use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use op_task::{Abbreviation, Status, Task, Timestamp};

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
            metadata: Metadata::from_partial(partial.metadata, &partial.conflicts, abbreviation),
        }
    }

    pub fn from_task(id: String, task: &Task, abbreviation: Abbreviation) -> Self {
        Self {
            id,
            title: task.title().unwrap_or_default(),
            metadata: Metadata::from_task(task, abbreviation),
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
        let metadata = Metadata::from_partial(partial.metadata, &partial.conflicts, abbreviation);
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
            metadata: Metadata::from_task(task, abbreviation),
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

// Something wrong with a task that no single write caught. Sync joins two versions that were each
// fine, and a hand edit in a local store passes no check, so the index looks at every task after
// each change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Problem {
    pub code: ProblemCode,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProblemCode {
    Field,
    Title,
    Comment,
    Diagram,
    Reference,
    Tag,
    ParentCycle,
    DependencyCycle,
    DuplicateNumber,
}

impl ProblemCode {
    pub fn as_str(self) -> &'static str {
        match self {
            ProblemCode::Field => "field",
            ProblemCode::Title => "title",
            ProblemCode::Comment => "comment",
            ProblemCode::Diagram => "diagram",
            ProblemCode::Reference => "reference",
            ProblemCode::Tag => "tag",
            ProblemCode::ParentCycle => "parent_cycle",
            ProblemCode::DependencyCycle => "dependency_cycle",
            ProblemCode::DuplicateNumber => "duplicate_number",
        }
    }
}

// One task read for the detail page: `metadata` parsed field by field, so a file with one bad field
// still renders everything else and flags only what failed. `updated` is the time of the last
// revision that changed the task. `parent_title`, `children`, and `refs` carry the immediate
// hierarchy so the page renders from this one read. `depends_on` is what this task waits for, in
// the order the file lists it; `blocks` is every task that waits for this one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TaskDetail {
    pub project: String,
    pub id: String,
    pub title: String,
    pub metadata: Metadata,
    pub body: String,
    // The open conflicts sync left in the task: fields in `metadata`, and blocks of both versions
    // in `body`.
    pub conflicts: usize,
    pub problems: Vec<Problem>,
    pub updated: Field<Rfc3339>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub author: Option<Author>,
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

// One task as a list row. `project` is the coordinate the key alone cannot carry: two projects can
// use the same abbreviation, so `id` names a task only within its project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TaskListItem {
    pub project: String,
    pub id: String,
    pub title: String,
    pub metadata: Metadata,
    // How many entries the log holds, not the log itself: a row shows a count, and shipping every
    // comment of every task would make the list read carry the whole store's prose.
    pub comment_count: usize,
    // Counted like `TaskDetail::conflicts`, so a row can call for attention to a conflict in the body.
    pub conflicts: usize,
    pub problems: Vec<Problem>,
    pub updated: Field<Rfc3339>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub author: Option<Author>,
}

// Who wrote the revision that created the task. Absent where the daemon did not read back as far as
// that revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Author {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub email: Option<String>,
    // The coding agent that created the task for `name`; absent when a person did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub agent: Option<String>,
}

// Which part of a task the query matched, and the order the hits come back in. A reader who types a
// key wants that task, not every task that names it, so the strongest match leads. Named apart from
// a task's `rank`, which is the order somebody set on the board.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SearchMatch {
    Key,
    Title,
    Text,
}

// One task a search matched, as the list row every other list read answers with, so a hit renders
// exactly like a row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SearchHit {
    pub task: TaskListItem,
    pub matched: SearchMatch,
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
