use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::MatrixCell;

// What the rolling-updates branch has that the default branch does not, and what stopped it. The
// two fields carry facts rather than a state a client should render: "nothing to publish" is an
// empty `pending`, and "blocked" is a `conflict`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RollingUpdates {
    pub pending: Vec<MatrixCell>,
    pub conflict: Option<Conflict>,
}

// A rebase that stopped. The files keep their markers in `worktree`, so a person fixes them there
// and runs `git rebase --continue`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Conflict {
    pub files: Vec<String>,
    pub worktree: String,
}

// The branch the remote now holds, and the pull request that merges it. `pull_request` is empty
// when `gh` is absent and the remote is not GitHub, which leaves the branch name as the only lead.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Published {
    pub remote: String,
    pub branch: String,
    pub commit: String,
    pub pull_request: Option<String>,
}
