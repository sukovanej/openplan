use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChangeEvent {
    // Created, edited, commented on, renumbered, or deleted. A client reads the task again and
    // learns which.
    TaskChanged { project: String, id: String },
    // A tag was registered, recolored, re-described, renamed, or deleted.
    TagsChanged { project: String },
    // Membership, a rename, a status change, or a new abbreviation.
    ProjectsChanged,
    // A sync with the remote ran, whether it moved anything or failed.
    SyncChanged { project: String },
    // An agent session started, ended, changed its status, opened or closed an approval, or wrote a
    // task. A client answers every one of those by reading the session list again.
    AgentSessionsChanged { project: String },
    // The stream dropped events and cannot say which, so the client reads everything on screen again.
    Resync,
    DaemonStopping,
}
