use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::event::ItemId;

// Opaque to the caller: each backend puts its own routing key here, and only that backend reads it
// back.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ApprovalId(pub String);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApprovalRequest {
    pub id: ApprovalId,
    pub item: Option<ItemId>,
    pub tool: String,
    pub input: Value,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    Allow,
    AllowForSession,
    Deny { reason: Option<String> },
    Abort,
}
