use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use op_forge::Forge;

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub sync: Option<SyncView>,
    // The repository of the remote the project syncs with; absent when the remote is not on GitHub
    // or GitLab.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub forge: Option<Forge>,
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

// `from` is absent where the tasks already had `version`, the store version of this daemon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Migration {
    pub project: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub from: Option<String>,
    pub version: String,
}
