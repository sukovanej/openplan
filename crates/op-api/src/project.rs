use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::history::SyncView;

// One project the daemon serves. `git_common_dir` is what a client matches its own checkout against:
// it is the same for every worktree of a repository. A project on a local directory has none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ProjectView {
    pub name: String,
    pub root: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub git_common_dir: Option<String>,
    pub backend: BackendKind,
    pub abbreviation: String,
    pub status: ProjectStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub sync: Option<SyncView>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum BackendKind {
    Git,
    Local,
}

impl BackendKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Git => "git",
            Self::Local => "local",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ProjectStatus {
    Ok,
    Error { reason: String },
}

impl ProjectStatus {
    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Ok => None,
            Self::Error { reason } => Some(reason),
        }
    }
}

// `backend` and `abbreviation` start a project that has no tasks yet. Without them the daemon serves
// what the path already holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RegisterProject {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub backend: Option<BackendKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub abbreviation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RenameProject {
    pub name: String,
}
