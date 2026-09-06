use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{approval::ApprovalRequest, session::SessionId, usage::Usage};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ItemId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TurnId(pub String);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "event")]
pub enum AgentEvent {
    Ready(SessionInfo),
    TurnStarted {
        turn: TurnId,
    },
    Thinking {
        item: ItemId,
        delta: String,
    },
    ThinkingEnded {
        item: ItemId,
        text: String,
    },
    Message {
        item: ItemId,
        delta: String,
    },
    MessageEnded {
        item: ItemId,
        text: String,
    },
    ResultDelta {
        delta: String,
    },
    ResultReady {
        value: Value,
    },
    ToolStarted(ToolCall),
    ToolOutput {
        item: ItemId,
        delta: String,
    },
    ToolEnded(ToolOutcome),
    ApprovalRequested(ApprovalRequest),
    UsageUpdated {
        turn: Usage,
        session: Usage,
        context_window: Option<u64>,
    },
    RateLimit(RateLimit),
    BudgetExhausted {
        session: Usage,
    },
    TurnEnded {
        turn: TurnId,
        stop: TurnStop,
    },
    Failed {
        message: String,
        retrying: bool,
    },
    Exited {
        code: Option<i32>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionInfo {
    pub session: SessionId,
    pub cwd: PathBuf,
    pub model: Option<String>,
    pub tools: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub item: ItemId,
    pub name: String,
    pub input: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolOutcome {
    pub item: ItemId,
    pub output: String,
    pub failed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnStop {
    Completed,
    Interrupted,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RateLimit {
    pub windows: Vec<RateLimitWindow>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RateLimitWindow {
    pub name: String,
    pub used_percent: f64,
    pub resets_at: Option<i64>,
}
