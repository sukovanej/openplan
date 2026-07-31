use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Extra, Nullable, content::Message, nullable};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Record {
    Known(Box<TranscriptRecord>),
    Unknown(Value),
}

impl Record {
    pub fn known(&self) -> Option<&TranscriptRecord> {
        match self {
            Self::Known(record) => Some(record),
            Self::Unknown(_) => None,
        }
    }

    pub fn message(&self) -> Option<&Message> {
        match self.known()? {
            TranscriptRecord::Assistant { message, .. }
            | TranscriptRecord::User { message, .. } => Some(message),
            _ => None,
        }
    }
}

// Only the turn records carry the full journalling envelope; the session-scoped bookkeeping records
// below it are lean and share nothing but `sessionId`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Envelope {
    pub uuid: String,
    pub session_id: String,
    pub timestamp: String,
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub parent_uuid: Nullable<String>,
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub is_sidechain: Nullable<bool>,
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub user_type: Nullable<String>,
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub entrypoint: Nullable<String>,
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub cwd: Nullable<String>,
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub version: Nullable<String>,
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub git_branch: Nullable<String>,
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub agent_id: Nullable<String>,
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub slug: Nullable<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum TranscriptRecord {
    Assistant {
        #[serde(flatten)]
        envelope: Envelope,
        message: Message,
        #[serde(
            rename = "requestId",
            default,
            deserialize_with = "nullable",
            skip_serializing_if = "Option::is_none"
        )]
        request_id: Nullable<String>,
        #[serde(flatten, default, skip_serializing_if = "Extra::is_empty")]
        extra: Extra,
    },
    User {
        #[serde(flatten)]
        envelope: Envelope,
        message: Message,
        #[serde(
            rename = "toolUseResult",
            default,
            deserialize_with = "nullable",
            skip_serializing_if = "Option::is_none"
        )]
        tool_use_result: Nullable<Value>,
        #[serde(flatten, default, skip_serializing_if = "Extra::is_empty")]
        extra: Extra,
    },
    Attachment {
        #[serde(flatten)]
        envelope: Envelope,
        attachment: Value,
        #[serde(flatten, default, skip_serializing_if = "Extra::is_empty")]
        extra: Extra,
    },
    System {
        #[serde(flatten)]
        envelope: Envelope,
        subtype: String,
        #[serde(
            default,
            deserialize_with = "nullable",
            skip_serializing_if = "Option::is_none"
        )]
        content: Nullable<Value>,
        #[serde(flatten, default, skip_serializing_if = "Extra::is_empty")]
        extra: Extra,
    },
    Mode {
        #[serde(rename = "sessionId")]
        session_id: String,
        mode: String,
        #[serde(flatten, default, skip_serializing_if = "Extra::is_empty")]
        extra: Extra,
    },
    PermissionMode {
        #[serde(rename = "sessionId")]
        session_id: String,
        #[serde(rename = "permissionMode")]
        permission_mode: String,
        #[serde(flatten, default, skip_serializing_if = "Extra::is_empty")]
        extra: Extra,
    },
    LastPrompt {
        #[serde(rename = "sessionId")]
        session_id: String,
        #[serde(rename = "leafUuid")]
        leaf_uuid: String,
        #[serde(
            rename = "lastPrompt",
            default,
            deserialize_with = "nullable",
            skip_serializing_if = "Option::is_none"
        )]
        last_prompt: Nullable<String>,
        #[serde(flatten, default, skip_serializing_if = "Extra::is_empty")]
        extra: Extra,
    },
    AiTitle {
        #[serde(rename = "sessionId")]
        session_id: String,
        #[serde(rename = "aiTitle")]
        ai_title: String,
        #[serde(flatten, default, skip_serializing_if = "Extra::is_empty")]
        extra: Extra,
    },
    CustomTitle {
        #[serde(rename = "sessionId")]
        session_id: String,
        #[serde(rename = "customTitle")]
        custom_title: String,
        #[serde(flatten, default, skip_serializing_if = "Extra::is_empty")]
        extra: Extra,
    },
    FileHistorySnapshot {
        #[serde(rename = "messageId")]
        message_id: String,
        snapshot: Value,
        #[serde(
            rename = "isSnapshotUpdate",
            default,
            deserialize_with = "nullable",
            skip_serializing_if = "Option::is_none"
        )]
        is_snapshot_update: Nullable<bool>,
        #[serde(flatten, default, skip_serializing_if = "Extra::is_empty")]
        extra: Extra,
    },
    FileHistoryDelta {
        #[serde(rename = "messageId")]
        message_id: String,
        #[serde(rename = "snapshotMessageId")]
        snapshot_message_id: String,
        #[serde(rename = "trackingPath")]
        tracking_path: Value,
        #[serde(flatten, default, skip_serializing_if = "Extra::is_empty")]
        extra: Extra,
    },
    QueueOperation {
        #[serde(rename = "sessionId")]
        session_id: String,
        operation: String,
        #[serde(
            default,
            deserialize_with = "nullable",
            skip_serializing_if = "Option::is_none"
        )]
        content: Nullable<Value>,
        #[serde(flatten, default, skip_serializing_if = "Extra::is_empty")]
        extra: Extra,
    },
    WorktreeState {
        #[serde(rename = "sessionId")]
        session_id: String,
        #[serde(rename = "worktreeSession")]
        worktree_session: Value,
        #[serde(flatten, default, skip_serializing_if = "Extra::is_empty")]
        extra: Extra,
    },
    Relocated {
        #[serde(rename = "sessionId")]
        session_id: String,
        #[serde(rename = "relocatedCwd")]
        relocated_cwd: String,
        #[serde(flatten, default, skip_serializing_if = "Extra::is_empty")]
        extra: Extra,
    },
    PrLink {
        #[serde(rename = "sessionId")]
        session_id: String,
        #[serde(rename = "prNumber")]
        pr_number: Value,
        #[serde(rename = "prUrl")]
        pr_url: String,
        #[serde(flatten, default, skip_serializing_if = "Extra::is_empty")]
        extra: Extra,
    },
    FrameLink {
        #[serde(rename = "sessionId")]
        session_id: String,
        #[serde(rename = "frameUrl")]
        frame_url: String,
        #[serde(flatten, default, skip_serializing_if = "Extra::is_empty")]
        extra: Extra,
    },
    // `started` and `result` only appear in workflow journals, where they bracket one agent run.
    // Note the tag `result` collides with the stream protocol's terminal record, which is a
    // different shape entirely — see `stream::StreamOutput::Result`.
    Started {
        key: String,
        #[serde(rename = "agentId")]
        agent_id: String,
        #[serde(flatten, default, skip_serializing_if = "Extra::is_empty")]
        extra: Extra,
    },
    Result {
        key: String,
        #[serde(rename = "agentId")]
        agent_id: String,
        result: Value,
        #[serde(flatten, default, skip_serializing_if = "Extra::is_empty")]
        extra: Extra,
    },
}
