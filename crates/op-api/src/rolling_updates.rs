use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::MatrixCell;

// What the rolling-updates branch has that the default branch does not, and what stopped it. The
// two fields carry facts rather than a state a client should render: "nothing to publish" is an
// empty `pending`, and "blocked" is a `conflict`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
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

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Published {
    pub branch: String,
    pub commit: String,
}
