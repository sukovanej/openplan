use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Presence {
    pub task_id: String,
    pub actor: String,
    pub worktree: String,
    pub branch: String,
    pub last_heartbeat_secs: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChangeEvent {
    TaskChanged {
        project: String,
        id: String,
        branch: String,
    },
    RefMoved {
        project: String,
        branch: String,
    },
    PresenceChanged {
        project: String,
        task_id: String,
    },
    // A tag was registered, recolored, re-described, renamed, or deleted. A client answers all five
    // the same way — read the registry again — so they are one event rather than five.
    TagsChanged {
        project: String,
        branch: String,
    },
    // Membership, a rename, a status change, or a new abbreviation. A client answers all four the
    // same way — read `/api/projects` again — so they are one event rather than four.
    ProjectsChanged,
    // The rolling-updates branch committed, rebased, published, or stopped at a conflict. A client
    // answers every one of those by reading the sync status again.
    SyncChanged {
        project: String,
    },
    // The stream dropped events and cannot say which, so the client re-reads everything on screen.
    Resync,
    DaemonStopping,
}
