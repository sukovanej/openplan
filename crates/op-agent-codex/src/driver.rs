use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};

use op_agent::{AgentEvent, Command, SessionOptions, Usage, process::Process, session::Channel};
use serde_json::{Value, json};
use tokio::{
    sync::mpsc::Sender,
    time::{Duration, Instant, sleep_until},
};

// How long a closed stdin has to end the CLI before it is killed.
const GRACE: Duration = Duration::from_secs(5);

use crate::{
    launch,
    protocol::{Incoming, KnownNotification, Notification, Outgoing, RequestId},
    translate::{self, Translator},
};

enum Pending {
    Initialize,
    ThreadStart,
}

struct Approval {
    request: RequestId,
    // Codex offers a different set per prompt, and a decision it did not offer is an error.
    available: Vec<String>,
}

pub async fn run(
    process: Process<Outgoing, Incoming>,
    mut channel: Channel,
    options: SessionOptions,
) {
    let Process {
        mut incoming,
        outgoing,
        mut child,
    } = process;
    let mut outgoing = Some(outgoing);
    let mut translator = Translator::new(options.cwd.clone(), options.model.clone());
    let mut requests = 0u64;
    let mut pending: HashMap<u64, Pending> = HashMap::new();
    let mut approvals: HashMap<String, Approval> = HashMap::new();
    let mut queued: Vec<String> = Vec::new();
    let mut session = Usage::default();
    let mut over_budget = false;
    let mut reading = true;
    let mut deadline: Option<Instant> = None;

    let id = next(&mut requests);
    pending.insert(id, Pending::Initialize);
    send(
        &outgoing,
        Outgoing::request(id, "initialize", launch::initialize()),
    )
    .await;

    loop {
        tokio::select! {
            message = incoming.recv(), if reading => {
                let Some(message) = message else {
                    reading = false;
                    continue;
                };
                match message {
                    Incoming::Result { id, result } => {
                        let RequestId::Number(id) = id else { continue };
                        match pending.remove(&(id as u64)) {
                            Some(Pending::Initialize) => {
                                send(&outgoing, Outgoing::notification("initialized")).await;
                                let (method, params) = launch::thread(&options);
                                let id = next(&mut requests);
                                pending.insert(id, Pending::ThreadStart);
                                send(&outgoing, Outgoing::request(id, method, params)).await;
                            }
                            Some(Pending::ThreadStart) => {
                                let Some(thread) = result.pointer("/thread/id").and_then(Value::as_str) else {
                                    continue;
                                };
                                let ready = translator.thread_started(thread.to_owned());
                                if !channel.emit(ready).await {
                                    return;
                                }
                                for text in queued.drain(..) {
                                    start_turn(&outgoing, &mut requests, thread, text, &options).await;
                                }
                            }
                            None => {}
                        }
                    }
                    Incoming::Failure { error, .. } => {
                        let failed = AgentEvent::Failed { message: error.message, retrying: false };
                        if !channel.emit(failed).await {
                            return;
                        }
                    }
                    Incoming::Request { id, method, params } => {
                        match translate::approval_request(&id, &method, &params) {
                            Some(request) => {
                                let approval = Approval {
                                    request: id,
                                    available: available(&params),
                                };
                                approvals.insert(request.id.0.clone(), approval);
                                if !channel.emit(AgentEvent::ApprovalRequested(request)).await {
                                    return;
                                }
                            }
                            None => send(&outgoing, answer(id, &method)).await,
                        }
                    }
                    Incoming::Notification(notification) => {
                        if let Some(resolved) = resolved(&notification) {
                            approvals.remove(&resolved);
                        }
                        for event in translator.translate(&notification) {
                            if let AgentEvent::UsageUpdated { session: total, .. } = &event {
                                session = *total;
                            }
                            if !channel.emit(event).await {
                                return;
                            }
                        }
                        if !over_budget && options.budget.exhausted_by(&session) {
                            over_budget = true;
                            if !channel.emit(AgentEvent::BudgetExhausted { session }).await {
                                return;
                            }
                            interrupt(&outgoing, &mut requests, &translator).await;
                        }
                    }
                }
            }
            command = channel.commands.recv() => {
                let Some(command) = command else {
                    outgoing = None;
                    deadline = Some(Instant::now() + GRACE);
                    continue;
                };
                match command {
                    Command::Prompt(text) => {
                        if over_budget {
                            let refused = AgentEvent::Failed {
                                message: "the session has spent its budget".to_owned(),
                                retrying: false,
                            };
                            if !channel.emit(refused).await {
                                return;
                            }
                            continue;
                        }
                        match translator.thread() {
                            // The thread is still starting, so the prompt waits for its id.
                            None => queued.push(text),
                            Some(thread) => {
                                let thread = thread.to_owned();
                                start_turn(&outgoing, &mut requests, &thread, text, &options).await;
                            }
                        }
                    }
                    Command::Interrupt => interrupt(&outgoing, &mut requests, &translator).await,
                    Command::Approve { id, decision } => {
                        if let Some(approval) = approvals.remove(&id.0) {
                            let verdict = translate::decision(&decision, &approval.available);
                            let result = json!({ "decision": verdict });
                            send(&outgoing, Outgoing::response(approval.request, result)).await;
                        }
                    }
                    // Closing stdin ends the CLI, and the exit arm below reports it.
                    Command::Shutdown => {
                        outgoing = None;
                        deadline = Some(Instant::now() + GRACE);
                    }
                }
            }
            _ = sleep_until(deadline.unwrap_or_else(Instant::now)), if deadline.is_some() => {
                deadline = None;
                let _ = child.start_kill();
            }
            status = child.wait() => {
                let code = status.ok().and_then(|status| status.code());
                channel.emit(AgentEvent::Exited { code }).await;
                return;
            }
        }
    }
}

fn next(requests: &mut u64) -> u64 {
    *requests += 1;
    *requests
}

async fn send(outgoing: &Option<Sender<Outgoing>>, message: Outgoing) {
    if let Some(outgoing) = outgoing {
        let _ = outgoing.send(message).await;
    }
}

async fn start_turn(
    outgoing: &Option<Sender<Outgoing>>,
    requests: &mut u64,
    thread: &str,
    text: String,
    options: &SessionOptions,
) {
    let params = launch::turn(thread, text, options);
    send(
        outgoing,
        Outgoing::request(next(requests), "turn/start", params),
    )
    .await;
}

async fn interrupt(
    outgoing: &Option<Sender<Outgoing>>,
    requests: &mut u64,
    translator: &Translator,
) {
    let (Some(thread), Some(turn)) = (translator.thread(), translator.turn()) else {
        return;
    };
    let params = json!({ "threadId": thread, "turnId": turn });
    send(
        outgoing,
        Outgoing::request(next(requests), "turn/interrupt", params),
    )
    .await;
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

fn available(params: &Value) -> Vec<String> {
    params
        .get("availableDecisions")
        .and_then(Value::as_array)
        .map(|decisions| {
            decisions
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn resolved(notification: &Notification) -> Option<String> {
    let Notification::Known(known) = notification else {
        return None;
    };
    match known.as_ref() {
        KnownNotification::ServerRequestResolved { request_id } => {
            Some(translate::request_key(request_id))
        }
        _ => None,
    }
}
