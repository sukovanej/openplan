use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct HistoryEntry {
    pub revision: RevisionView,
    pub changes: Vec<DocumentChange>,
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
