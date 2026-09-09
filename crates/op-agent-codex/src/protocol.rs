use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestId {
    Number(i64),
    Text(String),
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum Outgoing {
    Request {
        jsonrpc: &'static str,
        id: u64,
        method: &'static str,
        params: Value,
    },
    Notification {
        jsonrpc: &'static str,
        method: &'static str,
    },
    Response {
        jsonrpc: &'static str,
        id: RequestId,
        result: Value,
    },
    Failure {
        jsonrpc: &'static str,
        id: RequestId,
        error: RpcError,
    },
}

const VERSION: &str = "2.0";

impl Outgoing {
    pub fn request(id: u64, method: &'static str, params: Value) -> Self {
        Self::Request {
            jsonrpc: VERSION,
            id,
            method,
            params,
        }
    }

    pub fn notification(method: &'static str) -> Self {
        Self::Notification {
            jsonrpc: VERSION,
            method,
        }
    }

    pub fn response(id: RequestId, result: Value) -> Self {
        Self::Response {
            jsonrpc: VERSION,
            id,
            result,
        }
    }

    pub fn unsupported(id: RequestId, method: &str) -> Self {
        Self::Failure {
            jsonrpc: VERSION,
            id,
            error: RpcError {
                code: -32601,
                message: format!("openplan does not answer `{method}`"),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
}

// The variants are ordered by how tightly they match: a response carries `result` or `error`, a
// request carries `id` beside `method`, and a notification is what is left.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Incoming {
    Failure {
        id: RequestId,
        error: RpcError,
    },
    Result {
        id: RequestId,
        result: Value,
    },
    Request {
        id: RequestId,
        method: String,
        params: Value,
    },
    Notification(Notification),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Notification {
    Known(Box<KnownNotification>),
    Unknown(Value),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "method", content = "params", rename_all_fields = "camelCase")]
pub enum KnownNotification {
    #[serde(rename = "thread/started")]
    ThreadStarted { thread: Thread },
    #[serde(rename = "turn/started")]
    TurnStarted { turn: Turn },
    #[serde(rename = "turn/completed")]
    TurnCompleted { turn: Turn },
    #[serde(rename = "item/started")]
    ItemStarted { item: ThreadItem },
    #[serde(rename = "item/completed")]
    ItemCompleted { item: ThreadItem },
    #[serde(rename = "item/agentMessage/delta")]
    MessageDelta { item_id: String, delta: String },
    #[serde(rename = "item/reasoning/summaryTextDelta")]
    ReasoningDelta { item_id: String, delta: String },
    #[serde(rename = "item/commandExecution/outputDelta")]
    CommandOutputDelta { item_id: String, delta: String },
    #[serde(rename = "thread/tokenUsage/updated")]
    TokenUsage { token_usage: ThreadTokenUsage },
    #[serde(rename = "account/rateLimits/updated")]
    RateLimits { rate_limits: RateLimitSnapshot },
    #[serde(rename = "serverRequest/resolved")]
    ServerRequestResolved { request_id: RequestId },
    #[serde(rename = "error")]
    Error { error: TurnError, will_retry: bool },
}

#[derive(Debug, Clone, Deserialize)]
pub struct Thread {
    pub id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Turn {
    pub id: String,
    pub status: TurnStatus,
    #[serde(default)]
    pub error: Option<TurnError>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TurnStatus {
    InProgress,
    Completed,
    Interrupted,
    Failed,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TurnError {
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadTokenUsage {
    pub last: TokenUsageBreakdown,
    pub total: TokenUsageBreakdown,
    #[serde(default)]
    pub model_context_window: Option<u64>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsageBreakdown {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    #[serde(default)]
    pub cache_write_input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_output_tokens: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitSnapshot {
    #[serde(default)]
    pub primary: Option<RateLimitWindow>,
    #[serde(default)]
    pub secondary: Option<RateLimitWindow>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitWindow {
    pub used_percent: f64,
    #[serde(default)]
    pub resets_at: Option<i64>,
    #[serde(default)]
    pub window_duration_mins: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ThreadItem {
    Known(Box<KnownThreadItem>),
    Other(Value),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
pub enum KnownThreadItem {
    #[serde(rename = "agentMessage")]
    AgentMessage {
        id: String,
        text: String,
        // `commentary` for the notes the agent writes while it works, `final_answer` for the answer
        // itself. An output schema shapes both, so only the phase tells them apart.
        #[serde(default)]
        phase: Option<String>,
    },
    #[serde(rename = "reasoning")]
    Reasoning {
        id: String,
        #[serde(default)]
        summary: Vec<String>,
        #[serde(default)]
        content: Vec<String>,
    },
    #[serde(rename = "commandExecution")]
    CommandExecution {
        id: String,
        command: String,
        #[serde(default)]
        cwd: Option<String>,
        #[serde(default)]
        aggregated_output: Option<String>,
        #[serde(default)]
        exit_code: Option<i32>,
        status: ItemStatus,
    },
    #[serde(rename = "fileChange")]
    FileChange {
        id: String,
        changes: Vec<FileUpdateChange>,
        status: ItemStatus,
    },
    #[serde(rename = "mcpToolCall")]
    McpToolCall {
        id: String,
        server: String,
        tool: String,
        #[serde(default)]
        arguments: Value,
        #[serde(default)]
        error: Option<TurnError>,
        #[serde(default)]
        result: Option<Value>,
        status: ItemStatus,
    },
    #[serde(rename = "webSearch")]
    WebSearch { id: String, query: String },
    #[serde(rename = "userMessage")]
    UserMessage { id: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ItemStatus {
    InProgress,
    Completed,
    Failed,
    Declined,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FileUpdateChange {
    pub path: String,
    pub kind: PatchChangeKind,
    pub diff: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PatchChangeKind {
    Add,
    Delete,
    Update {
        #[serde(default)]
        move_path: Option<String>,
    },
}
