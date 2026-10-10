use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

// Something the daemon cannot fix by itself. It lasts until a person fixes its cause.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema)]
pub struct Fault {
    pub project: String,
    pub kind: FaultKind,
    pub message: String,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum FaultKind {
    // The daemon serves nothing of a project whose root is gone.
    RootGone,
    // The tasks or the project config do not parse.
    Unreadable,
    // The tasks use a store version that only a newer openplan reads. The daemon serves none of
    // them, and it looks for an update.
    NewerStoreVersion,
    // The tasks use an older store version that this daemon does not migrate by itself. It serves
    // them, and it refuses each write.
    OlderStoreVersion,
    // Git `user.name` signs every write, so with none set, no write is made.
    NoIdentity,
    SyncFailed,
    // Another process wrote the tasks, or a person edited the files by hand, and the daemon could
    // not take the change in.
    OutsideChangesUnread,
}
