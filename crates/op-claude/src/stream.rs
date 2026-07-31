use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Extra, Nullable, content::Message, nullable};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StreamOutput {
    Known(Box<KnownStreamOutput>),
    Unknown(Value),
}

impl StreamOutput {
    pub fn known(&self) -> Option<&KnownStreamOutput> {
        match self {
            Self::Known(output) => Some(output),
            Self::Unknown(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum KnownStreamOutput {
    System {
        subtype: String,
        session_id: String,
        uuid: String,
        #[serde(flatten, default, skip_serializing_if = "Extra::is_empty")]
        extra: Extra,
    },
    Assistant {
        message: Message,
        session_id: String,
        uuid: String,
        #[serde(
            default,
            deserialize_with = "nullable",
            skip_serializing_if = "Option::is_none"
        )]
        parent_tool_use_id: Nullable<String>,
        #[serde(
            default,
            deserialize_with = "nullable",
            skip_serializing_if = "Option::is_none"
        )]
        request_id: Nullable<String>,
        #[serde(flatten, default, skip_serializing_if = "Extra::is_empty")]
        extra: Extra,
    },
    User {
        message: Message,
        session_id: String,
        uuid: String,
        #[serde(
            default,
            deserialize_with = "nullable",
            skip_serializing_if = "Option::is_none"
        )]
        parent_tool_use_id: Nullable<String>,
        // Set only under --replay-user-messages, which echoes back what the caller wrote to stdin.
        #[serde(
            rename = "isReplay",
            default,
            deserialize_with = "nullable",
            skip_serializing_if = "Option::is_none"
        )]
        is_replay: Nullable<bool>,
        #[serde(flatten, default, skip_serializing_if = "Extra::is_empty")]
        extra: Extra,
    },
    // Raw Messages API SSE frames, forwarded verbatim under --include-partial-messages.
    StreamEvent {
        event: Value,
        session_id: String,
        uuid: String,
        #[serde(
            default,
            deserialize_with = "nullable",
            skip_serializing_if = "Option::is_none"
        )]
        parent_tool_use_id: Nullable<String>,
        #[serde(flatten, default, skip_serializing_if = "Extra::is_empty")]
        extra: Extra,
    },
    RateLimitEvent {
        rate_limit_info: Value,
        session_id: String,
        uuid: String,
        #[serde(flatten, default, skip_serializing_if = "Extra::is_empty")]
        extra: Extra,
    },
    ControlRequest {
        request_id: String,
        request: Value,
        #[serde(flatten, default, skip_serializing_if = "Extra::is_empty")]
        extra: Extra,
    },
    ControlResponse {
        response: Value,
        #[serde(flatten, default, skip_serializing_if = "Extra::is_empty")]
        extra: Extra,
    },
    Result {
        subtype: String,
        session_id: String,
        uuid: String,
        is_error: bool,
        num_turns: u32,
        duration_ms: u64,
        #[serde(
            default,
            deserialize_with = "nullable",
            skip_serializing_if = "Option::is_none"
        )]
        result: Nullable<String>,
        #[serde(
            default,
            deserialize_with = "nullable",
            skip_serializing_if = "Option::is_none"
        )]
        total_cost_usd: Nullable<f64>,
        #[serde(
            default,
            deserialize_with = "nullable",
            skip_serializing_if = "Option::is_none"
        )]
        usage: Nullable<Value>,
        #[serde(flatten, default, skip_serializing_if = "Extra::is_empty")]
        extra: Extra,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StreamInput {
    Known(Box<KnownStreamInput>),
    Unknown(Value),
}

impl StreamInput {
    pub fn user_text(text: impl Into<String>) -> Self {
        use crate::content::{ContentBlock, KnownContentBlock, MessageContent, Role};

        Self::Known(Box::new(KnownStreamInput::User {
            message: Box::new(Message {
                role: Role::User,
                content: MessageContent::Blocks(vec![ContentBlock::Known(
                    KnownContentBlock::Text {
                        text: text.into(),
                        extra: Extra::new(),
                    },
                )]),
                id: None,
                model: None,
                stop_reason: None,
                stop_sequence: None,
                usage: None,
                extra: Extra::new(),
            }),
            parent_tool_use_id: None,
            session_id: None,
            extra: Extra::new(),
        }))
    }

    pub fn known(&self) -> Option<&KnownStreamInput> {
        match self {
            Self::Known(input) => Some(input),
            Self::Unknown(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum KnownStreamInput {
    User {
        message: Box<Message>,
        #[serde(
            default,
            deserialize_with = "nullable",
            skip_serializing_if = "Option::is_none"
        )]
        parent_tool_use_id: Nullable<String>,
        #[serde(
            default,
            deserialize_with = "nullable",
            skip_serializing_if = "Option::is_none"
        )]
        session_id: Nullable<String>,
        #[serde(flatten, default, skip_serializing_if = "Extra::is_empty")]
        extra: Extra,
    },
    ControlRequest {
        request_id: String,
        request: Value,
        #[serde(flatten, default, skip_serializing_if = "Extra::is_empty")]
        extra: Extra,
    },
    ControlResponse {
        response: Value,
        #[serde(flatten, default, skip_serializing_if = "Extra::is_empty")]
        extra: Extra,
    },
}
