use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    approval::ApprovalRequest,
    event::{AgentEvent, ItemId, RateLimit, SessionInfo, ToolCall, ToolOutcome, TurnId},
    usage::Usage,
};

// The state of a session as far as the events have told it: what a reader who joins late needs
// to draw the same view as a reader who saw every event.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Transcript {
    pub info: Option<SessionInfo>,
    pub status: Status,
    pub turn: Option<TurnId>,
    pub entries: Vec<Entry>,
    pub approvals: Vec<ApprovalRequest>,
    // The structured answer of the running turn as far as it has been written; `result` holds the
    // last one that ended.
    pub result_text: String,
    pub result: Option<Value>,
    pub usage: Usage,
    pub context_window: Option<u64>,
    pub rate_limit: Option<RateLimit>,
    pub over_budget: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Status {
    #[default]
    Starting,
    Idle,
    Running,
    Exited {
        code: Option<i32>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Entry {
    Prompt {
        text: String,
    },
    Thinking {
        item: ItemId,
        text: String,
        done: bool,
    },
    Message {
        item: ItemId,
        text: String,
        done: bool,
    },
    Tool {
        item: ItemId,
        name: String,
        input: Value,
        output: String,
        done: bool,
        failed: bool,
    },
    Failure {
        message: String,
        retrying: bool,
    },
}

impl Transcript {
    pub fn apply(&mut self, event: &AgentEvent) {
        match event {
            AgentEvent::Ready(info) => {
                self.info = Some(info.clone());
                if self.status == Status::Starting {
                    self.status = Status::Idle;
                }
            }
            AgentEvent::TurnStarted { turn, prompt } => {
                self.status = Status::Running;
                self.turn = Some(turn.clone());
                self.result_text.clear();
                self.entries.push(Entry::Prompt {
                    text: prompt.clone(),
                });
            }
            AgentEvent::Thinking { item, delta } => {
                self.text(item, true).push_str(delta);
            }
            AgentEvent::ThinkingEnded { item, text } => {
                *self.text(item, true) = text.clone();
                self.finish(item);
            }
            AgentEvent::Message { item, delta } => {
                self.text(item, false).push_str(delta);
            }
            AgentEvent::MessageEnded { item, text } => {
                *self.text(item, false) = text.clone();
                self.finish(item);
            }
            AgentEvent::ResultDelta { delta } => self.result_text.push_str(delta),
            AgentEvent::ResultReady { value } => self.result = Some(value.clone()),
            AgentEvent::ToolStarted(ToolCall { item, name, input }) => {
                self.entries.push(Entry::Tool {
                    item: item.clone(),
                    name: name.clone(),
                    input: input.clone(),
                    output: String::new(),
                    done: false,
                    failed: false,
                });
            }
            AgentEvent::ToolOutput { item, delta } => {
                if let Some(Entry::Tool { output, .. }) = self.entry(item) {
                    output.push_str(delta);
                }
            }
            AgentEvent::ToolEnded(outcome) => self.tool_ended(outcome),
            AgentEvent::ApprovalRequested(request) => self.approvals.push(request.clone()),
            AgentEvent::ApprovalResolved { id } => {
                self.approvals.retain(|request| &request.id != id);
            }
            AgentEvent::UsageUpdated {
                session,
                context_window,
                ..
            } => {
                self.usage = *session;
                self.context_window = *context_window;
            }
            AgentEvent::RateLimit(limit) => self.rate_limit = Some(limit.clone()),
            AgentEvent::BudgetExhausted { session } => {
                self.usage = *session;
                self.over_budget = true;
            }
            AgentEvent::TurnEnded { .. } => self.settle(Status::Idle),
            AgentEvent::Failed { message, retrying } => self.entries.push(Entry::Failure {
                message: message.clone(),
                retrying: *retrying,
            }),
            AgentEvent::Exited { code } => self.settle(Status::Exited { code: *code }),
        }
    }

    pub fn running(&self) -> bool {
        self.status == Status::Running
    }

    fn entry(&mut self, item: &ItemId) -> Option<&mut Entry> {
        self.entries
            .iter_mut()
            .rev()
            .find(|entry| entry.item() == Some(item))
    }

    fn text(&mut self, item: &ItemId, thinking: bool) -> &mut String {
        let found = self
            .entries
            .iter()
            .rposition(|entry| entry.item() == Some(item));
        let position = found.unwrap_or_else(|| {
            self.entries.push(Entry::text(item.clone(), thinking));
            self.entries.len() - 1
        });
        self.entries[position].text_mut()
    }

    fn finish(&mut self, item: &ItemId) {
        if let Some(entry) = self.entry(item) {
            entry.finish();
        }
    }

    fn tool_ended(&mut self, outcome: &ToolOutcome) {
        let Some(Entry::Tool {
            output,
            done,
            failed,
            ..
        }) = self.entry(&outcome.item)
        else {
            return;
        };
        // A backend that streamed the output reports the same text again at the end, and one
        // that did not stream it reports it only here; an empty report keeps what streamed.
        if !outcome.output.is_empty() {
            *output = outcome.output.clone();
        }
        *done = true;
        *failed = outcome.failed;
    }

    fn settle(&mut self, status: Status) {
        self.status = status;
        self.turn = None;
        self.approvals.clear();
        for entry in &mut self.entries {
            entry.finish();
        }
    }
}

impl Entry {
    pub fn item(&self) -> Option<&ItemId> {
        match self {
            Self::Thinking { item, .. } | Self::Message { item, .. } | Self::Tool { item, .. } => {
                Some(item)
            }
            Self::Prompt { .. } | Self::Failure { .. } => None,
        }
    }

    fn text(item: ItemId, thinking: bool) -> Self {
        let text = String::new();
        if thinking {
            Self::Thinking {
                item,
                text,
                done: false,
            }
        } else {
            Self::Message {
                item,
                text,
                done: false,
            }
        }
    }

    fn text_mut(&mut self) -> &mut String {
        match self {
            Self::Prompt { text } | Self::Thinking { text, .. } | Self::Message { text, .. } => {
                text
            }
            Self::Tool { output, .. } => output,
            Self::Failure { message, .. } => message,
        }
    }

    fn finish(&mut self) {
        match self {
            Self::Thinking { done, .. } | Self::Message { done, .. } | Self::Tool { done, .. } => {
                *done = true;
            }
            Self::Prompt { .. } | Self::Failure { .. } => {}
        }
    }
}
