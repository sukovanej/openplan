use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::MatrixCell;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SyncState {
    #[default]
    InSync,
    Pending,
    Syncing,
    Blocked,
}

// What the rolling-updates lane holds and how it is faring. `pending` is the lane's diff against
// the default branch, and `conflicted` names the files a stopped rebase left for a person to fix in
// `worktree`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SyncStatus {
    pub state: SyncState,
    pub pending: Vec<MatrixCell>,
    pub conflicted: Vec<String>,
    pub worktree: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Published {
    pub branch: String,
    pub commit: String,
}
