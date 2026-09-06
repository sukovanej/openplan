use std::{collections::BTreeSet, path::PathBuf};

use op_agent::{
    AgentEvent, ApprovalDecision, ApprovalId, ApprovalRequest, ItemId, RateLimit, RateLimitWindow,
    SessionId, SessionInfo, ToolCall, ToolOutcome, TurnId, TurnStop, Usage,
};

use serde_json::{Value, json};

use crate::protocol::{
    FileUpdateChange, ItemStatus, KnownNotification, KnownThreadItem, Notification,
    RateLimitSnapshot, RateLimitWindow as CodexWindow, RequestId, ThreadItem, TokenUsageBreakdown,
    Turn, TurnStatus,
};

pub fn approval_request(id: &RequestId, method: &str, params: &Value) -> Option<ApprovalRequest> {
    let (tool, input) = match method {
        "item/commandExecution/requestApproval" => (
            "shell",
            json!({ "command": params.get("command"), "cwd": params.get("cwd") }),
        ),
        "item/fileChange/requestApproval" => ("apply_patch", params.clone()),
        _ => return None,
    };
    Some(ApprovalRequest {
        id: ApprovalId(request_key(id)),
        item: params
            .get("itemId")
            .and_then(Value::as_str)
            .map(|item| ItemId(item.to_owned())),
        tool: tool.to_owned(),
        input,
        reason: params
            .get("reason")
            .and_then(Value::as_str)
            .map(str::to_owned),
    })
}

pub fn request_key(id: &RequestId) -> String {
    match id {
        RequestId::Number(number) => number.to_string(),
        RequestId::Text(text) => text.clone(),
    }
}

pub fn decision(decision: &ApprovalDecision, available: &[String]) -> &'static str {
    let (wanted, fallback) = match decision {
        ApprovalDecision::Allow => ("accept", "accept"),
        ApprovalDecision::AllowForSession => ("acceptForSession", "accept"),
        ApprovalDecision::Deny { .. } => ("decline", "cancel"),
        ApprovalDecision::Abort => ("cancel", "cancel"),
    };
    if available.is_empty() || available.iter().any(|offered| offered == wanted) {
        wanted
    } else {
        fallback
    }
}

pub struct Translator {
    cwd: PathBuf,
    model: Option<String>,
    // With an output schema the answer of the turn is the value itself, so the agent message
    // carries JSON rather than prose.
    structured: bool,
    answers: BTreeSet<String>,
    thread: Option<String>,
    turn: Option<String>,
}

impl Translator {
    pub fn new(cwd: PathBuf, model: Option<String>, structured: bool) -> Self {
        Self {
            cwd,
            model,
            structured,
            answers: BTreeSet::new(),
            thread: None,
            turn: None,
        }
    }

    pub fn thread(&self) -> Option<&str> {
        self.thread.as_deref()
    }

    pub fn turn(&self) -> Option<&str> {
        self.turn.as_deref()
    }

    pub fn thread_started(&mut self, thread: String) -> AgentEvent {
        self.thread = Some(thread.clone());
        AgentEvent::Ready(SessionInfo {
            session: SessionId(thread),
            cwd: self.cwd.clone(),
            model: self.model.clone(),
            tools: Vec::new(),
        })
    }

    pub fn translate(&mut self, notification: &Notification) -> Vec<AgentEvent> {
        let Notification::Known(known) = notification else {
            return Vec::new();
        };
        match known.as_ref() {
            // The thread id also arrives as the reply to `thread/start`, whichever lands first.
            KnownNotification::ThreadStarted { thread } => {
                if self.thread.is_some() {
                    Vec::new()
                } else {
                    vec![self.thread_started(thread.id.clone())]
                }
            }
            KnownNotification::TurnStarted { turn } => {
                self.turn = Some(turn.id.clone());
                vec![AgentEvent::TurnStarted {
                    turn: TurnId(turn.id.clone()),
                }]
            }
            KnownNotification::TurnCompleted { turn } => self.turn_completed(turn),
            KnownNotification::ItemStarted { item } => {
                if let Some(answer) = self.answer_id(item) {
                    self.answers.insert(answer);
                }
                item_started(item)
            }
            KnownNotification::ItemCompleted { item } => {
                let answer = self.answer_id(item).is_some();
                item_completed(item, answer)
            }
            KnownNotification::MessageDelta { item_id, delta }
                if self.answers.contains(item_id) =>
            {
                vec![AgentEvent::ResultDelta {
                    delta: delta.clone(),
                }]
            }
            KnownNotification::MessageDelta { item_id, delta } => vec![AgentEvent::Message {
                item: ItemId(item_id.clone()),
                delta: delta.clone(),
            }],
            KnownNotification::ReasoningDelta { item_id, delta } => vec![AgentEvent::Thinking {
                item: ItemId(item_id.clone()),
                delta: delta.clone(),
            }],
            KnownNotification::CommandOutputDelta { item_id, delta } => {
                vec![AgentEvent::ToolOutput {
                    item: ItemId(item_id.clone()),
                    delta: delta.clone(),
                }]
            }
            KnownNotification::TokenUsage { token_usage } => vec![AgentEvent::UsageUpdated {
                turn: usage(&token_usage.last),
                session: usage(&token_usage.total),
                context_window: token_usage.model_context_window,
            }],
            KnownNotification::RateLimits { rate_limits } => {
                vec![AgentEvent::RateLimit(rate_limit(rate_limits))]
            }
            KnownNotification::Error { error, will_retry } => vec![AgentEvent::Failed {
                message: error.message.clone(),
                retrying: *will_retry,
            }],
            KnownNotification::ServerRequestResolved { .. } => Vec::new(),
        }
    }

    fn answer_id(&self, item: &ThreadItem) -> Option<String> {
        let ThreadItem::Known(known) = item else {
            return None;
        };
        match known.as_ref() {
            KnownThreadItem::AgentMessage { id, phase, .. }
                if self.structured && phase.as_deref() == Some(FINAL_ANSWER) =>
            {
                Some(id.clone())
            }
            _ => None,
        }
    }

    fn turn_completed(&mut self, turn: &Turn) -> Vec<AgentEvent> {
        self.turn = None;
        self.answers.clear();
        let stop = match turn.status {
            TurnStatus::Completed | TurnStatus::InProgress => TurnStop::Completed,
            TurnStatus::Interrupted => TurnStop::Interrupted,
            TurnStatus::Failed => TurnStop::Failed,
        };
        let mut events = Vec::new();
        if let Some(error) = &turn.error {
            events.push(AgentEvent::Failed {
                message: error.message.clone(),
                retrying: false,
            });
        }
        events.push(AgentEvent::TurnEnded {
            turn: TurnId(turn.id.clone()),
            stop,
        });
        events
    }
}

fn item_started(item: &ThreadItem) -> Vec<AgentEvent> {
    let known = match item {
        ThreadItem::Known(known) => known.as_ref(),
        ThreadItem::Other(value) => return other_item(value).into_iter().collect(),
    };
    let call = match known {
        KnownThreadItem::CommandExecution {
            id, command, cwd, ..
        } => ToolCall {
            item: ItemId(id.clone()),
            name: "shell".to_owned(),
            input: json!({ "command": command, "cwd": cwd }),
        },
        KnownThreadItem::FileChange { id, changes, .. } => ToolCall {
            item: ItemId(id.clone()),
            name: "apply_patch".to_owned(),
            input: json!({ "paths": paths(changes) }),
        },
        KnownThreadItem::McpToolCall {
            id,
            server,
            tool,
            arguments,
            ..
        } => ToolCall {
            item: ItemId(id.clone()),
            name: format!("{server}/{tool}"),
            input: arguments.clone(),
        },
        KnownThreadItem::WebSearch { id, query } => ToolCall {
            item: ItemId(id.clone()),
            name: "web_search".to_owned(),
            input: json!({ "query": query }),
        },
        // Text arrives as deltas and is reported whole when the item completes.
        KnownThreadItem::AgentMessage { .. }
        | KnownThreadItem::Reasoning { .. }
        | KnownThreadItem::UserMessage { .. } => return Vec::new(),
    };
    vec![AgentEvent::ToolStarted(call)]
}

const FINAL_ANSWER: &str = "final_answer";

fn item_completed(item: &ThreadItem, answer: bool) -> Vec<AgentEvent> {
    let known = match item {
        ThreadItem::Known(known) => known.as_ref(),
        ThreadItem::Other(value) => {
            return other_id(value)
                .map(|item| {
                    AgentEvent::ToolEnded(ToolOutcome {
                        item,
                        output: String::new(),
                        failed: false,
                    })
                })
                .into_iter()
                .collect();
        }
    };
    match known {
        KnownThreadItem::AgentMessage { text, .. } if answer => serde_json::from_str(text)
            .map(|value| AgentEvent::ResultReady { value })
            .into_iter()
            .collect(),
        KnownThreadItem::AgentMessage { id, text, .. } => vec![AgentEvent::MessageEnded {
            item: ItemId(id.clone()),
            text: text.clone(),
        }],
        KnownThreadItem::Reasoning {
            id,
            summary,
            content,
        } => {
            let text = if summary.is_empty() {
                content.join("\n")
            } else {
                summary.join("\n")
            };
            if text.is_empty() {
                Vec::new()
            } else {
                vec![AgentEvent::ThinkingEnded {
                    item: ItemId(id.clone()),
                    text,
                }]
            }
        }
        KnownThreadItem::CommandExecution {
            id,
            aggregated_output,
            status,
            ..
        } => vec![AgentEvent::ToolEnded(ToolOutcome {
            item: ItemId(id.clone()),
            output: aggregated_output.clone().unwrap_or_default(),
            failed: failed(*status),
        })],
        KnownThreadItem::FileChange {
            id,
            changes,
            status,
        } => vec![AgentEvent::ToolEnded(ToolOutcome {
            item: ItemId(id.clone()),
            output: changes
                .iter()
                .map(|change| change.diff.as_str())
                .collect::<Vec<_>>()
                .join("\n"),
            failed: failed(*status),
        })],
        KnownThreadItem::McpToolCall {
            id,
            error,
            result,
            status,
            ..
        } => vec![AgentEvent::ToolEnded(ToolOutcome {
            item: ItemId(id.clone()),
            output: match (error, result) {
                (Some(error), _) => error.message.clone(),
                (None, Some(result)) => result.to_string(),
                (None, None) => String::new(),
            },
            failed: failed(*status),
        })],
        KnownThreadItem::WebSearch { id, query } => vec![AgentEvent::ToolEnded(ToolOutcome {
            item: ItemId(id.clone()),
            output: query.clone(),
            failed: false,
        })],
        KnownThreadItem::UserMessage { .. } => Vec::new(),
    }
}

// Codex adds item types between releases; an unmodelled one still belongs on the stream, named by
// its own `type`, rather than disappearing.
fn other_item(value: &Value) -> Option<AgentEvent> {
    let item = other_id(value)?;
    let name = value.get("type").and_then(Value::as_str)?;
    Some(AgentEvent::ToolStarted(ToolCall {
        item,
        name: name.to_owned(),
        input: value.clone(),
    }))
}

fn other_id(value: &Value) -> Option<ItemId> {
    value
        .get("id")
        .and_then(Value::as_str)
        .map(|id| ItemId(id.to_owned()))
}

fn failed(status: ItemStatus) -> bool {
    matches!(status, ItemStatus::Failed | ItemStatus::Declined)
}

fn paths(changes: &[FileUpdateChange]) -> Vec<&str> {
    changes.iter().map(|change| change.path.as_str()).collect()
}

fn usage(breakdown: &TokenUsageBreakdown) -> Usage {
    Usage {
        input_tokens: breakdown.input_tokens,
        cached_input_tokens: breakdown.cached_input_tokens,
        cache_write_tokens: breakdown.cache_write_input_tokens,
        output_tokens: breakdown.output_tokens,
        reasoning_tokens: breakdown.reasoning_output_tokens,
        cost_usd: None,
    }
}

fn rate_limit(snapshot: &RateLimitSnapshot) -> RateLimit {
    let window = |name: &str, window: Option<&CodexWindow>| {
        window.map(|window| RateLimitWindow {
            name: name.to_owned(),
            used_percent: window.used_percent,
            resets_at: window.resets_at,
        })
    };
    RateLimit {
        windows: window("primary", snapshot.primary.as_ref())
            .into_iter()
            .chain(window("secondary", snapshot.secondary.as_ref()))
            .collect(),
    }
}
