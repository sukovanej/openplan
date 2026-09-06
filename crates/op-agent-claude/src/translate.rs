use op_agent::{
    AgentEvent, ApprovalId, ApprovalRequest, ItemId, RateLimit, RateLimitWindow, SessionId,
    SessionInfo, ToolCall, ToolOutcome, TurnId, TurnStop, Usage,
};
use std::collections::BTreeSet;

use op_claude::{ContentBlock, KnownContentBlock, KnownStreamOutput, Message, StreamOutput};
use serde_json::Value;

// The CLI answers `--json-schema` with a built-in tool of this name, and streams the value as that
// tool's arguments. Nothing else marks the structured answer apart from an ordinary tool call.
const STRUCTURED: &str = "StructuredOutput";

pub struct Translator {
    ready: bool,
    interrupted: bool,
    turn: u64,
    message: Option<String>,
    structured: BTreeSet<usize>,
    session: Usage,
    // Claude Code reports cost for the whole session, so a turn costs the difference.
    spent: f64,
}

impl Default for Translator {
    fn default() -> Self {
        Self::new()
    }
}

impl Translator {
    pub fn new() -> Self {
        Self {
            ready: false,
            interrupted: false,
            turn: 0,
            message: None,
            structured: BTreeSet::new(),
            session: Usage::default(),
            spent: 0.0,
        }
    }

    pub fn session_usage(&self) -> Usage {
        self.session
    }

    // The CLI reports an interrupted turn as a failure with a `terminal_reason` that also covers
    // aborts nobody asked for, so the driver says when it stopped the turn itself.
    pub fn interrupted(&mut self) {
        self.interrupted = true;
    }

    pub fn turn_started(&mut self) -> AgentEvent {
        self.interrupted = false;
        self.structured.clear();
        self.turn += 1;
        AgentEvent::TurnStarted {
            turn: self.turn_id(),
        }
    }

    pub fn translate(&mut self, output: &StreamOutput) -> Vec<AgentEvent> {
        let Some(known) = output.known() else {
            return Vec::new();
        };
        match known {
            KnownStreamOutput::System {
                subtype,
                session_id,
                extra,
                ..
            } => self.system(subtype, session_id, extra),
            KnownStreamOutput::Assistant { message, .. } => self.assistant(message),
            KnownStreamOutput::User { message, .. } => tool_results(message),
            KnownStreamOutput::StreamEvent { event, .. } => self.stream_event(event),
            KnownStreamOutput::RateLimitEvent {
                rate_limit_info, ..
            } => vec![AgentEvent::RateLimit(rate_limit(rate_limit_info))],
            KnownStreamOutput::ControlRequest {
                request_id,
                request,
                ..
            } => approval(request_id, request).into_iter().collect(),
            KnownStreamOutput::Result {
                subtype,
                is_error,
                result,
                total_cost_usd,
                usage,
                ..
            } => self.result(
                subtype,
                *is_error,
                result.clone().flatten(),
                total_cost_usd.flatten(),
                usage.clone().flatten(),
            ),
            KnownStreamOutput::ControlResponse { .. } => Vec::new(),
        }
    }

    fn turn_id(&self) -> TurnId {
        TurnId(self.turn.to_string())
    }

    // The CLI re-announces the session at the head of every turn; only the first one is news.
    fn system(
        &mut self,
        subtype: &str,
        session_id: &str,
        extra: &op_claude::Extra,
    ) -> Vec<AgentEvent> {
        if subtype != "init" || self.ready {
            return Vec::new();
        }
        self.ready = true;
        vec![AgentEvent::Ready(SessionInfo {
            session: SessionId(session_id.to_owned()),
            cwd: extra
                .get("cwd")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .into(),
            model: extra
                .get("model")
                .and_then(Value::as_str)
                .map(str::to_owned),
            tools: extra
                .get("tools")
                .and_then(Value::as_array)
                .map(|tools| {
                    tools
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default(),
        })]
    }

    fn assistant(&mut self, message: &Message) -> Vec<AgentEvent> {
        let id = message.id.clone().flatten().unwrap_or_default();
        message
            .content
            .blocks()
            .iter()
            .enumerate()
            .filter_map(|(index, block)| {
                let ContentBlock::Known(block) = block else {
                    return None;
                };
                match block {
                    KnownContentBlock::Text { text, .. } => Some(AgentEvent::MessageEnded {
                        item: block_id(&id, index),
                        text: text.clone(),
                    }),
                    // The complete record redacts the thinking text and keeps only its signature,
                    // so the deltas are the only place the text appears.
                    KnownContentBlock::Thinking { thinking, .. } if !thinking.is_empty() => {
                        Some(AgentEvent::ThinkingEnded {
                            item: block_id(&id, index),
                            text: thinking.clone(),
                        })
                    }
                    // The structured answer reaches the caller as `ResultReady`, not as a call the
                    // caller never asked for.
                    KnownContentBlock::ToolUse { name, .. } if name == STRUCTURED => None,
                    KnownContentBlock::ToolUse {
                        id, name, input, ..
                    } => Some(AgentEvent::ToolStarted(ToolCall {
                        item: ItemId(id.clone()),
                        name: name.clone(),
                        input: input.clone(),
                    })),
                    _ => None,
                }
            })
            .collect()
    }

    fn stream_event(&mut self, event: &Value) -> Vec<AgentEvent> {
        match event.get("type").and_then(Value::as_str) {
            Some("message_start") => {
                self.message = event
                    .pointer("/message/id")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                Vec::new()
            }
            Some("content_block_start") => {
                if event.pointer("/content_block/name").and_then(Value::as_str) == Some(STRUCTURED)
                {
                    let index = event
                        .get("index")
                        .and_then(Value::as_u64)
                        .unwrap_or_default();
                    self.structured.insert(index as usize);
                }
                Vec::new()
            }
            Some("content_block_delta") => {
                let Some(message) = &self.message else {
                    return Vec::new();
                };
                let index = event.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
                let item = block_id(message, index);
                let delta = event.get("delta");
                match delta
                    .and_then(|delta| delta.get("type"))
                    .and_then(Value::as_str)
                {
                    Some("text_delta") => vec![AgentEvent::Message {
                        item,
                        delta: text_at(delta, "text"),
                    }],
                    Some("thinking_delta") => vec![AgentEvent::Thinking {
                        item,
                        delta: text_at(delta, "thinking"),
                    }],
                    Some("input_json_delta") if self.structured.contains(&index) => {
                        vec![AgentEvent::ResultDelta {
                            delta: text_at(delta, "partial_json"),
                        }]
                    }
                    _ => Vec::new(),
                }
            }
            _ => Vec::new(),
        }
    }

    fn result(
        &mut self,
        subtype: &str,
        is_error: bool,
        message: Option<String>,
        total_cost_usd: Option<f64>,
        usage: Option<Value>,
    ) -> Vec<AgentEvent> {
        let cost = total_cost_usd.map(|total| {
            let turn = total - self.spent;
            self.spent = total;
            turn
        });
        let turn = turn_usage(usage.as_ref().unwrap_or(&Value::Null), cost);
        self.session.add(&turn);

        let stop = match (self.interrupted, is_error) {
            (true, _) => TurnStop::Interrupted,
            (false, true) => TurnStop::Failed,
            (false, false) => TurnStop::Completed,
        };
        let mut events = Vec::new();
        if !self.structured.is_empty()
            && let Some(value) = message
                .as_deref()
                .and_then(|message| serde_json::from_str(message).ok())
        {
            events.push(AgentEvent::ResultReady { value });
        }
        events.push(AgentEvent::UsageUpdated {
            turn,
            session: self.session,
            context_window: None,
        });
        if stop == TurnStop::Failed {
            events.push(AgentEvent::Failed {
                message: message.unwrap_or_else(|| subtype.to_owned()),
                retrying: false,
            });
        }
        events.push(AgentEvent::TurnEnded {
            turn: self.turn_id(),
            stop,
        });
        events
    }
}

fn block_id(message: &str, index: usize) -> ItemId {
    ItemId(format!("{message}#{index}"))
}

fn text_at(delta: Option<&Value>, field: &str) -> String {
    delta
        .and_then(|delta| delta.get(field))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn tool_results(message: &Message) -> Vec<AgentEvent> {
    message
        .content
        .blocks()
        .iter()
        .filter_map(|block| {
            let ContentBlock::Known(KnownContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
                ..
            }) = block
            else {
                return None;
            };
            Some(AgentEvent::ToolEnded(ToolOutcome {
                item: ItemId(tool_use_id.clone()),
                output: flatten_content(content.clone().flatten().as_ref()),
                failed: is_error.flatten().unwrap_or(false),
            }))
        })
        .collect()
}

// A tool result is a bare string on one call and a block list on the next.
fn flatten_content(content: Option<&Value>) -> String {
    match content {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(blocks)) => blocks
            .iter()
            .filter_map(|block| block.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n"),
        Some(other) => other.to_string(),
    }
}

fn approval(request_id: &str, request: &Value) -> Option<AgentEvent> {
    if request.get("subtype").and_then(Value::as_str) != Some("can_use_tool") {
        return None;
    }
    Some(AgentEvent::ApprovalRequested(ApprovalRequest {
        id: ApprovalId(request_id.to_owned()),
        item: request
            .get("tool_use_id")
            .and_then(Value::as_str)
            .map(|id| ItemId(id.to_owned())),
        tool: request
            .get("tool_name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        input: request.get("input").cloned().unwrap_or(Value::Null),
        reason: request
            .get("reason")
            .or_else(|| request.get("blocked_path"))
            .and_then(Value::as_str)
            .map(str::to_owned),
    }))
}

fn rate_limit(info: &Value) -> RateLimit {
    let windows = info
        .get("unifiedWindows")
        .and_then(Value::as_object)
        .map(|windows| {
            windows
                .iter()
                .map(|(name, window)| RateLimitWindow {
                    name: name.clone(),
                    used_percent: window
                        .get("utilization")
                        .and_then(Value::as_f64)
                        .unwrap_or_default()
                        * 100.0,
                    resets_at: window.get("resetsAt").and_then(Value::as_i64),
                })
                .collect()
        })
        .unwrap_or_default();
    RateLimit { windows }
}

fn turn_usage(usage: &Value, cost_usd: Option<f64>) -> Usage {
    let count = |field: &str| usage.get(field).and_then(Value::as_u64).unwrap_or_default();
    let cached_input_tokens = count("cache_read_input_tokens");
    Usage {
        input_tokens: count("input_tokens") + cached_input_tokens,
        cached_input_tokens,
        cache_write_tokens: count("cache_creation_input_tokens"),
        output_tokens: count("output_tokens"),
        reasoning_tokens: usage
            .pointer("/output_tokens_details/thinking_tokens")
            .and_then(Value::as_u64)
            .unwrap_or_default(),
        cost_usd,
    }
}
