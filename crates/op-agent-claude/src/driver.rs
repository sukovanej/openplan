use std::collections::{BTreeSet, HashMap};

use op_agent::{
    AgentEvent, ApprovalDecision, ApprovalId, Effects, ItemId, Protocol, ToolCall, TurnId,
    TurnStop, Usage,
};
use op_claude::{
    ContentBlock, Extra, KnownContentBlock, KnownStreamInput, KnownStreamOutput, Message,
    StreamInput, StreamOutput,
};
use serde_json::Value;

use crate::translate::{
    approval, block_id, control_response, interrupt, rate_limit, session_info, text_at,
    tool_results, turn_usage,
};

// The CLI answers `--json-schema` with a built-in tool of this name, and streams the value as that
// tool's arguments. Nothing else marks the structured answer apart from an ordinary tool call.
const STRUCTURED: &str = "StructuredOutput";

#[derive(Default)]
pub struct Driver {
    ready: bool,
    interrupted: bool,
    turn: u64,
    message: Option<String>,
    structured: BTreeSet<usize>,
    session: Usage,
    // Claude Code reports cost for the whole session, so a turn costs the difference.
    spent: f64,
    // A decision has to echo the tool input back, so each request waits here until it is answered.
    pending: HashMap<String, Value>,
    controls: u64,
}

impl Driver {
    pub fn new() -> Self {
        Self::default()
    }

    fn turn_id(&self) -> TurnId {
        TurnId(self.turn.to_string())
    }

    fn control(&mut self, request: Value) -> StreamInput {
        self.controls += 1;
        StreamInput::Known(Box::new(KnownStreamInput::ControlRequest {
            request_id: format!("op-{}", self.controls),
            request,
            extra: Extra::new(),
        }))
    }

    fn translate(&mut self, output: &StreamOutput) -> Vec<AgentEvent> {
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
            KnownStreamOutput::Assistant { message, .. } => assistant(message),
            KnownStreamOutput::User { message, .. } => tool_results(message),
            KnownStreamOutput::StreamEvent { event, .. } => self.stream_event(event),
            KnownStreamOutput::RateLimitEvent {
                rate_limit_info, ..
            } => vec![AgentEvent::RateLimit(rate_limit(rate_limit_info))],
            KnownStreamOutput::ControlRequest {
                request_id,
                request,
                ..
            } => {
                let Some(approval) = approval(request_id, request) else {
                    return Vec::new();
                };
                self.pending.insert(request_id.clone(), request.clone());
                vec![AgentEvent::ApprovalRequested(approval)]
            }
            KnownStreamOutput::Result {
                subtype,
                is_error,
                result,
                total_cost_usd,
                usage,
                extra,
                ..
            } => self.result(
                subtype,
                *is_error,
                result.clone().flatten(),
                total_cost_usd.flatten(),
                usage.clone().flatten(),
                extra,
            ),
            KnownStreamOutput::ControlResponse { .. } => Vec::new(),
        }
    }

    // The CLI re-announces the session at the head of every turn; only the first one is news.
    fn system(&mut self, subtype: &str, session_id: &str, extra: &Extra) -> Vec<AgentEvent> {
        if subtype != "init" || self.ready {
            return Vec::new();
        }
        self.ready = true;
        vec![AgentEvent::Ready(session_info(session_id, extra))]
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
                    self.structured.insert(index_of(event));
                }
                Vec::new()
            }
            Some("content_block_delta") => {
                let Some(message) = &self.message else {
                    return Vec::new();
                };
                let index = index_of(event);
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
        extra: &Extra,
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
            && let Some(value) = structured_output(extra, message.as_deref())
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

impl Protocol for Driver {
    type Input = StreamInput;
    type Output = StreamOutput;

    fn read(&mut self, output: StreamOutput, effects: &mut Effects<StreamInput>) {
        effects.events.extend(self.translate(&output));
    }

    fn prompt(&mut self, text: String, effects: &mut Effects<StreamInput>) {
        self.interrupted = false;
        self.structured.clear();
        self.turn += 1;
        effects.emit(AgentEvent::TurnStarted {
            turn: self.turn_id(),
            prompt: text.clone(),
        });
        effects.send(StreamInput::user_text(text));
    }

    // The CLI reports an interrupted turn as a failure with a `terminal_reason` that also covers
    // aborts nobody asked for, so the driver remembers that it stopped the turn itself.
    fn interrupt(&mut self, effects: &mut Effects<StreamInput>) {
        self.interrupted = true;
        let request = self.control(interrupt());
        effects.send(request);
    }

    fn approve(
        &mut self,
        id: ApprovalId,
        decision: ApprovalDecision,
        effects: &mut Effects<StreamInput>,
    ) {
        let request = self.pending.remove(&id.0);
        effects.send(control_response(&id, &decision, request.as_ref()));
        effects.emit(AgentEvent::ApprovalResolved { id });
        if decision == ApprovalDecision::Abort {
            self.interrupt(effects);
        }
    }

    fn usage(&self) -> Usage {
        self.session
    }
}

fn index_of(event: &Value) -> usize {
    event
        .get("index")
        .and_then(Value::as_u64)
        .unwrap_or_default() as usize
}

fn assistant(message: &Message) -> Vec<AgentEvent> {
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
                // The complete record redacts the thinking text and keeps only its signature, so
                // the deltas are the only place the text appears.
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

// Newer CLIs put the parsed value beside the text; older ones only quote it in `result`.
fn structured_output(extra: &Extra, message: Option<&str>) -> Option<Value> {
    match extra.get("structured_output") {
        Some(Value::Null) | None => message.and_then(|text| serde_json::from_str(text).ok()),
        Some(value) => Some(value.clone()),
    }
}
