use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

// One repository the daemon serves. `git_common_dir` is what a client matches its own checkout
// against: it is the same string for every worktree of a repository, where `root` names only the one
// checkout this daemon indexes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ProjectView {
    pub name: String,
    pub root: String,
    pub git_common_dir: String,
    pub abbreviation: String,
    pub status: ProjectStatus,
    // Named here so a client can label that branch without spelling its name.
    pub rolling_updates_branch: Option<String>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RegisterProject {
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RenameProject {
    pub name: String,
}
