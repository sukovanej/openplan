use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub const ADMIN_HEADER: &str = "x-openplan-admin";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ApiErrorBody {
    pub message: String,
    // The members of each dependency cycle a request could not order. A client links the keys, which
    // it cannot do with a sentence. Every other refusal sends the message alone.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cycles: Vec<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
// The daemon serves every registered repository, so it names none of them here. A client asks
// `/api/projects` which repository it is talking to.
pub struct DaemonInfo {
    pub pid: u32,
    pub port: u16,
    pub version: String,
    pub started_at: u64,
}
