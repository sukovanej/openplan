use std::{
    collections::{BTreeSet, HashMap, VecDeque},
    time::{SystemTime, UNIX_EPOCH},
};

use op_agent::{
    AgentEvent, ApprovalDecision, ApprovalId, Effects, ItemId, Protocol, SessionId, SessionInfo,
    SessionOptions, TurnId, TurnStop, Usage,
};
use serde_json::{Value, json};

use crate::{
    launch,
    protocol::{
        Incoming, KnownNotification, KnownThreadItem, Notification, Outgoing, RequestId,
        ThreadItem, Turn, TurnStatus,
    },
    translate::{self, item_completed, item_started},
};

// `commentary` marks the notes the agent writes while it works, `final_answer` the answer itself.
// An output schema shapes both, so only the phase tells them apart.
const FINAL_ANSWER: &str = "final_answer";

enum Pending {
    Initialize,
    ThreadStart,
}

struct Approval {
    request: RequestId,
    // Codex offers a different set per prompt, and a decision it did not offer is an error.
    available: Vec<String>,
}

pub struct Driver {
    options: SessionOptions,
    requests: u64,
    pending: HashMap<u64, Pending>,
    approvals: HashMap<String, Approval>,
    // Prompts that arrived before the thread had an id.
    queued: Vec<String>,
    // Prompts sent as turns, in order, until `turn/started` names each one.
    sent: VecDeque<String>,
    thread: Option<String>,
    turn: Option<String>,
    // The message items that carry the structured answer of the running turn.
    answers: BTreeSet<String>,
    // The server reports a fatal error as a notification and again on the turn it ended.
    failed: bool,
    usage: Usage,
}

impl Driver {
    pub fn new(options: SessionOptions) -> Self {
        Self {
            options,
            requests: 0,
            pending: HashMap::new(),
            approvals: HashMap::new(),
            queued: Vec::new(),
            sent: VecDeque::new(),
            thread: None,
            turn: None,
            answers: BTreeSet::new(),
            failed: false,
            usage: Usage::default(),
        }
    }

    pub fn thread(&self) -> Option<&str> {
        self.thread.as_deref()
    }

    pub fn turn(&self) -> Option<&str> {
        self.turn.as_deref()
    }

    fn request(&mut self, method: &'static str, params: Value) -> (u64, Outgoing) {
        self.requests += 1;
        (
            self.requests,
            Outgoing::request(self.requests, method, params),
        )
    }

    fn start_turn(&mut self, thread: &str, text: String, effects: &mut Effects<Outgoing>) {
        let params = launch::turn(thread, text.clone(), &self.options);
        let (_, request) = self.request("turn/start", params);
        self.sent.push_back(text);
        effects.send(request);
    }

    fn thread_started(&mut self, thread: String, effects: &mut Effects<Outgoing>) {
        if self.thread.is_some() {
            return;
        }
        self.thread = Some(thread.clone());
        effects.emit(AgentEvent::Ready(SessionInfo {
            session: SessionId(thread.clone()),
            cwd: self.options.cwd.clone(),
            model: self.options.model.clone(),
            tools: Vec::new(),
        }));
        for text in std::mem::take(&mut self.queued) {
            self.start_turn(&thread, text, effects);
        }
    }

    fn result(&mut self, id: RequestId, result: Value, effects: &mut Effects<Outgoing>) {
        let RequestId::Number(id) = id else { return };
        match self.pending.remove(&(id as u64)) {
            Some(Pending::Initialize) => {
                effects.send(Outgoing::notification("initialized"));
                let (method, params) = launch::thread(&self.options);
                let (id, request) = self.request(method, params);
                self.pending.insert(id, Pending::ThreadStart);
                effects.send(request);
            }
            Some(Pending::ThreadStart) => {
                if let Some(thread) = result.pointer("/thread/id").and_then(Value::as_str) {
                    self.thread_started(thread.to_owned(), effects);
                }
            }
            None => {}
        }
    }

    fn failure(&mut self, id: RequestId, message: String, effects: &mut Effects<Outgoing>) {
        effects.emit(AgentEvent::Failed {
            message,
            retrying: false,
        });
        // Without a thread nothing else can happen, so the session ends rather than waits.
        if let RequestId::Number(id) = id
            && self.pending.remove(&(id as u64)).is_some()
        {
            effects.close();
        }
    }

    fn server_request(
        &mut self,
        id: RequestId,
        method: &str,
        params: &Value,
        effects: &mut Effects<Outgoing>,
    ) {
        match translate::approval_request(&id, method, params) {
            Some(request) => {
                let approval = Approval {
                    request: id,
                    available: translate::available_decisions(params),
                };
                self.approvals.insert(request.id.0.clone(), approval);
                effects.emit(AgentEvent::ApprovalRequested(request));
            }
            None => effects.send(answer(id, method)),
        }
    }

    fn notification(&mut self, notification: Notification, effects: &mut Effects<Outgoing>) {
        let Notification::Known(known) = notification else {
            return;
        };
        match *known {
            // The thread id also arrives as the reply to `thread/start`, whichever lands first.
            KnownNotification::ThreadStarted { thread } => self.thread_started(thread.id, effects),
            KnownNotification::TurnStarted { turn } => {
                self.turn = Some(turn.id.clone());
                self.failed = false;
                effects.emit(AgentEvent::TurnStarted {
                    turn: TurnId(turn.id),
                    prompt: self.sent.pop_front().unwrap_or_default(),
                });
            }
            KnownNotification::TurnCompleted { turn } => self.turn_completed(turn, effects),
            KnownNotification::ItemStarted { item } => {
                if let Some(answer) = self.answer_id(&item) {
                    self.answers.insert(answer);
                }
                effects.events.extend(item_started(&item));
            }
            KnownNotification::ItemCompleted { item } => {
                let answer = self.answer_id(&item).is_some();
                effects.events.extend(item_completed(&item, answer));
            }
            KnownNotification::MessageDelta { item_id, delta } => {
                effects.emit(if self.answers.contains(&item_id) {
                    AgentEvent::ResultDelta { delta }
                } else {
                    AgentEvent::Message {
                        item: ItemId(item_id),
                        delta,
                    }
                });
            }
            KnownNotification::ReasoningDelta { item_id, delta } => {
                effects.emit(AgentEvent::Thinking {
                    item: ItemId(item_id),
                    delta,
                });
            }
            KnownNotification::CommandOutputDelta { item_id, delta } => {
                effects.emit(AgentEvent::ToolOutput {
                    item: ItemId(item_id),
                    delta,
                });
            }
            KnownNotification::TokenUsage { token_usage } => {
                self.usage = translate::usage(&token_usage.total);
                effects.emit(AgentEvent::UsageUpdated {
                    turn: translate::usage(&token_usage.last),
                    session: self.usage,
                    context_window: token_usage.model_context_window,
                });
            }
            KnownNotification::RateLimits { rate_limits } => {
                effects.emit(AgentEvent::RateLimit(translate::rate_limit(&rate_limits)));
            }
            KnownNotification::Error { error, will_retry } => {
                self.failed |= !will_retry;
                effects.emit(AgentEvent::Failed {
                    message: error.message,
                    retrying: will_retry,
                });
            }
            // The server also resolves a request on its own, on an interrupt for one.
            KnownNotification::ServerRequestResolved { request_id } => {
                let key = translate::request_key(&request_id);
                if self.approvals.remove(&key).is_some() {
                    effects.emit(AgentEvent::ApprovalResolved {
                        id: ApprovalId(key),
                    });
                }
            }
        }
    }

    fn answer_id(&self, item: &ThreadItem) -> Option<String> {
        let ThreadItem::Known(known) = item else {
            return None;
        };
        match known.as_ref() {
            KnownThreadItem::AgentMessage { id, phase, .. }
                if self.options.schema.is_some() && phase.as_deref() == Some(FINAL_ANSWER) =>
            {
                Some(id.clone())
            }
            _ => None,
        }
    }

    fn turn_completed(&mut self, turn: Turn, effects: &mut Effects<Outgoing>) {
        self.turn = None;
        self.answers.clear();
        let stop = match turn.status {
            TurnStatus::Completed | TurnStatus::InProgress => TurnStop::Completed,
            TurnStatus::Interrupted => TurnStop::Interrupted,
            TurnStatus::Failed => TurnStop::Failed,
        };
        if let Some(error) = turn.error
            && !self.failed
        {
            effects.emit(AgentEvent::Failed {
                message: error.message,
                retrying: false,
            });
        }
        effects.emit(AgentEvent::TurnEnded {
            turn: TurnId(turn.id),
            stop,
        });
    }
}

impl Protocol for Driver {
    type Input = Outgoing;
    type Output = Incoming;

    fn open(&mut self, effects: &mut Effects<Outgoing>) {
        let (id, request) = self.request("initialize", launch::initialize());
        self.pending.insert(id, Pending::Initialize);
        effects.send(request);
    }

    fn read(&mut self, message: Incoming, effects: &mut Effects<Outgoing>) {
        match message {
            Incoming::Result { id, result } => self.result(id, result, effects),
            Incoming::Failure { id, error } => self.failure(id, error.message, effects),
            Incoming::Request { id, method, params } => {
                self.server_request(id, &method, &params, effects);
            }
            Incoming::Notification(notification) => self.notification(notification, effects),
        }
    }

    fn prompt(&mut self, text: String, effects: &mut Effects<Outgoing>) {
        match self.thread.clone() {
            Some(thread) => self.start_turn(&thread, text, effects),
            // The thread is still starting, so the prompt waits for its id.
            None => self.queued.push(text),
        }
    }

    fn interrupt(&mut self, effects: &mut Effects<Outgoing>) {
        let (Some(thread), Some(turn)) = (&self.thread, &self.turn) else {
            return;
        };
        let params = json!({ "threadId": thread, "turnId": turn });
        let (_, request) = self.request("turn/interrupt", params);
        effects.send(request);
    }

    fn approve(
        &mut self,
        id: ApprovalId,
        decision: ApprovalDecision,
        effects: &mut Effects<Outgoing>,
    ) {
        let Some(approval) = self.approvals.remove(&id.0) else {
            return;
        };
        let verdict = translate::decision(&decision, &approval.available);
        let result = json!({ "decision": verdict });
        effects.send(Outgoing::response(approval.request, result));
        effects.emit(AgentEvent::ApprovalResolved { id });
    }

    fn usage(&self) -> Usage {
        self.usage
    }
}

// The server keeps a request open until it is answered, so an unhandled one is refused rather than
// left to stall the turn.
fn answer(id: RequestId, method: &str) -> Outgoing {
    match method {
        "currentTime/read" => {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|since| since.as_secs())
                .unwrap_or_default();
            Outgoing::response(id, json!({ "currentTimeAt": now }))
        }
        _ => Outgoing::unsupported(id, method),
    }
}
