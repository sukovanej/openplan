use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Extra, Nullable, nullable};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: MessageContent,
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub id: Nullable<String>,
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub model: Nullable<String>,
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub stop_reason: Nullable<String>,
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub stop_sequence: Nullable<String>,
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub usage: Nullable<Usage>,
    #[serde(flatten, default, skip_serializing_if = "Extra::is_empty")]
    pub extra: Extra,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

impl MessageContent {
    pub fn blocks(&self) -> &[ContentBlock] {
        match self {
            Self::Text(_) => &[],
            Self::Blocks(blocks) => blocks,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ContentBlock {
    Known(KnownContentBlock),
    Other(Value),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum KnownContentBlock {
    Text {
        text: String,
        #[serde(flatten, default, skip_serializing_if = "Extra::is_empty")]
        extra: Extra,
    },
    Thinking {
        thinking: String,
        #[serde(
            default,
            deserialize_with = "nullable",
            skip_serializing_if = "Option::is_none"
        )]
        signature: Nullable<String>,
        #[serde(flatten, default, skip_serializing_if = "Extra::is_empty")]
        extra: Extra,
    },
    RedactedThinking {
        data: String,
        #[serde(flatten, default, skip_serializing_if = "Extra::is_empty")]
        extra: Extra,
    },
    ToolUse {
        id: String,
        name: String,
        input: Value,
        #[serde(flatten, default, skip_serializing_if = "Extra::is_empty")]
        extra: Extra,
    },
    ToolResult {
        tool_use_id: String,
        #[serde(
            default,
            deserialize_with = "nullable",
            skip_serializing_if = "Option::is_none"
        )]
        content: Nullable<Value>,
        #[serde(
            default,
            deserialize_with = "nullable",
            skip_serializing_if = "Option::is_none"
        )]
        is_error: Nullable<bool>,
        #[serde(flatten, default, skip_serializing_if = "Extra::is_empty")]
        extra: Extra,
    },
    Image {
        source: Value,
        #[serde(flatten, default, skip_serializing_if = "Extra::is_empty")]
        extra: Extra,
    },
    Document {
        source: Value,
        #[serde(flatten, default, skip_serializing_if = "Extra::is_empty")]
        extra: Extra,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub input_tokens: Nullable<u64>,
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub output_tokens: Nullable<u64>,
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub cache_creation_input_tokens: Nullable<u64>,
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub cache_read_input_tokens: Nullable<u64>,
    #[serde(flatten, default, skip_serializing_if = "Extra::is_empty")]
    pub extra: Extra,
}
