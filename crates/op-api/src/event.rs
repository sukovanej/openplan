use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChangeEvent {
    // Created, edited, commented on, renumbered, or deleted. A client reads the task again and
    // learns which.
    TaskChanged { project: String, id: String },
    // A doc was created, edited, renamed, or deleted. A client reads the doc again and learns which.
    DocChanged { project: String, name: String },
    // A tag was registered, recolored, re-described, renamed, or deleted.
    TagsChanged { project: String },
    // Membership, a rename, a status change, or a new abbreviation.
    ProjectsChanged,
    // A sync with the remote ran, whether it moved anything or failed.
    SyncChanged { project: String },
    // A fault started or ended. A client reads the list of faults again.
    FaultsChanged,
    // The stream dropped events and cannot say which, so the client reads everything on screen again.
    Resync,
    DaemonStopping { reason: StopReason },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    Stop,
    // The daemon installed a new release and starts again on it, so a client reconnects at once.
    Update,
}
