use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub const ADMIN_HEADER: &str = "x-openplan-admin";

// What a caller can do about a refusal, where the status alone cannot say. A tag delete answers
// `TagReferenced` on the one 409 that `force` changes, and a task write answers `TagUnregistered` on
// the one 400 that registering the name changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Refusal {
    TagReferenced,
    TagUnregistered,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ApiErrorBody {
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub reason: Option<Refusal>,
    // The members of each dependency cycle a request could not order. A client links the keys, which
    // it cannot do with a sentence. Every other refusal sends the message alone.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cycles: Vec<Vec<String>>,
    // Where a diagram source stops parsing, so that an editor can point at the place.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub position: Option<SourcePosition>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SourcePosition {
    pub line: usize,
    pub column: usize,
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

// Who asked for a write. The CLI sends them on every request; a request without them, such as one
// from the web UI, is signed with the identity of the repository the daemon serves. Header values
// are ASCII, so each carries its text percent-encoded.
pub const AUTHOR_HEADER: &str = "x-openplan-author";
pub const EMAIL_HEADER: &str = "x-openplan-email";
pub const AGENT_HEADER: &str = "x-openplan-agent";

pub fn encode_header(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'!'..=b'~' if byte != b'%' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

pub fn decode_header(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        match bytes[at] {
            b'%' => {
                let hex = std::str::from_utf8(bytes.get(at + 1..at + 3)?).ok()?;
                out.push(u8::from_str_radix(hex, 16).ok()?);
                at += 3;
            }
            byte => {
                out.push(byte);
                at += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}
