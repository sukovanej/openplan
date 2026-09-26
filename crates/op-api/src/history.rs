use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::Status;
use crate::comment::Comment;
use crate::field::Rfc3339;
use crate::metadata::Metadata;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RevisionView {
    pub id: String,
    pub parents: Vec<String>,
    pub author: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub email: Option<String>,
    // The coding agent that wrote the revision for `author`; absent when a person wrote it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub agent: Option<String>,
    pub at: Rfc3339,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DocumentChangeKind {
    Added,
    Modified,
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct DocumentChange {
    pub path: String,
    pub kind: DocumentChangeKind,
    // The key of the task the document holds; absent for a tag, the config, or an asset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub task: Option<String>,
    // The name of the tag the document holds; absent for a task, the config, or an asset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub tag: Option<String>,
}

// The history reads what changed from the documents, never from the message, so a revision that
// another tool wrote is described as fully as one that openplan wrote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct HistoryEntry {
    pub revision: RevisionView,
    pub changes: Vec<DocumentChange>,
    // One line for each changed document, in the words of the commit messages that openplan writes.
    pub summary: Vec<String>,
    pub tasks: Vec<TaskChange>,
    pub tags: Vec<TagChange>,
}

// One task, even where a new title moved it to a file with a new name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TaskChange {
    pub task: String,
    pub kind: DocumentChangeKind,
    // For a removed task, the title it had before the revision.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub title: Option<String>,
    // Empty for an added or a removed task.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<FieldChange>,
}

// `parent`, `dependencies`, and `title` leave out the side that holds nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "field", rename_all = "snake_case")]
pub enum FieldChange {
    // A sync merge moved the task, because another task took its number first.
    Number {
        from: String,
        to: String,
    },
    Status {
        from: Status,
        to: Status,
    },
    Parent {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schema(nullable = false)]
        from: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schema(nullable = false)]
        to: Option<String>,
    },
    Order,
    Dependencies {
        from: Vec<String>,
        to: Vec<String>,
    },
    Tags {
        from: Vec<String>,
        to: Vec<String>,
    },
    Title {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schema(nullable = false)]
        from: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schema(nullable = false)]
        to: Option<String>,
    },
    Description,
    Comments {
        added: usize,
        removed: usize,
    },
    Conflicts {
        from: usize,
        to: usize,
    },
    // A field that openplan does not model, such as `created`, or a modeled field that one version
    // cannot read.
    Other {
        name: String,
    },
    // One version of the frontmatter does not parse, so no field of it can be compared.
    Frontmatter,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TagChange {
    pub tag: String,
    // A renamed tag is `modified`, and `renamed_from` holds its old name.
    pub kind: DocumentChangeKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub renamed_from: Option<String>,
}

// One task as it stood at one revision. `task` is absent where the task did not exist then.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TaskAtRevision {
    pub id: String,
    pub revision: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub task: Option<TaskSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TaskSnapshot {
    pub title: String,
    pub metadata: Metadata,
    pub body: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub comments: Vec<Comment>,
    pub raw: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SyncView {
    pub remote: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub last_attempt: Option<Rfc3339>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub last_success: Option<Rfc3339>,
    pub ahead: usize,
    pub behind: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SyncResult {
    pub received: usize,
    pub sent: usize,
    pub merged: bool,
    pub status: SyncView,
}
