use std::collections::HashMap;

use op_agent::{
    AgentEvent, ApprovalDecision, ApprovalId, Command, SessionOptions, process::Process,
    session::Channel,
};
use op_claude::{Extra, KnownStreamInput, KnownStreamOutput, StreamInput, StreamOutput};
use serde_json::{Value, json};
use tokio::{
    sync::mpsc::Sender,
    time::{Duration, Instant, sleep_until},
};

// How long a closed stdin has to end the CLI before it is killed.
const GRACE: Duration = Duration::from_secs(5);

use crate::translate::Translator;

pub async fn run(
    process: Process<StreamInput, StreamOutput>,
    mut channel: Channel,
    options: SessionOptions,
) {
    let Process {
        mut incoming,
        outgoing,
        mut child,
    } = process;
    let mut outgoing = Some(outgoing);
    let mut translator = Translator::new();
    let mut pending: HashMap<String, Value> = HashMap::new();
    let mut controls = 0u64;
    let mut over_budget = false;
    let mut reading = true;
    let mut deadline: Option<Instant> = None;

    loop {
        tokio::select! {
            output = incoming.recv(), if reading => {
                let Some(output) = output else {
                    reading = false;
                    continue;
                };
                remember_approval(&output, &mut pending);
                for event in translator.translate(&output) {
                    if !channel.emit(event).await {
                        return;
                    }
                }
                let session = translator.session_usage();
                if !over_budget && options.budget.exhausted_by(&session) {
                    over_budget = true;
                    if !channel.emit(AgentEvent::BudgetExhausted { session }).await {
                        return;
                    }
                    translator.interrupted();
                    send(&outgoing, interrupt(&mut controls)).await;
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
                        if !channel.emit(translator.turn_started()).await {
                            return;
                        }
                        send(&outgoing, StreamInput::user_text(text)).await;
                    }
                    Command::Interrupt => {
                        translator.interrupted();
                        send(&outgoing, interrupt(&mut controls)).await;
                    }
                    Command::Approve { id, decision } => {
                        let request = pending.remove(&id.0);
                        send(&outgoing, control_response(&id, &decision, request.as_ref())).await;
                        if decision == ApprovalDecision::Abort {
                            translator.interrupted();
                            send(&outgoing, interrupt(&mut controls)).await;
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

async fn send(outgoing: &Option<Sender<StreamInput>>, input: StreamInput) {
    if let Some(outgoing) = outgoing {
        let _ = outgoing.send(input).await;
    }
}

// A decision has to echo the tool input back, so the request is kept until it is answered.
fn remember_approval(output: &StreamOutput, pending: &mut HashMap<String, Value>) {
    if let Some(KnownStreamOutput::ControlRequest {
        request_id,
        request,
        ..
    }) = output.known()
        && request.get("subtype").and_then(Value::as_str) == Some("can_use_tool")
    {
        pending.insert(request_id.clone(), request.clone());
    }
}

fn interrupt(controls: &mut u64) -> StreamInput {
    *controls += 1;
    StreamInput::Known(Box::new(KnownStreamInput::ControlRequest {
        request_id: format!("op-{controls}"),
        request: json!({ "subtype": "interrupt" }),
        extra: Extra::new(),
    }))
}

fn control_response(
    id: &ApprovalId,
    decision: &ApprovalDecision,
    request: Option<&Value>,
) -> StreamInput {
    let input = request
        .and_then(|request| request.get("input"))
        .cloned()
        .unwrap_or(Value::Null);
    let outcome = match decision {
        ApprovalDecision::Allow => json!({ "behavior": "allow", "updatedInput": input }),
        ApprovalDecision::AllowForSession => {
            let mut allow = json!({ "behavior": "allow", "updatedInput": input });
            // The CLI writes the rules that would stop it asking again; sending them back is the
            // only way to widen the session's permissions from here.
            if let Some(rules) = request.and_then(|request| request.get("permission_suggestions")) {
                allow["updatedPermissions"] = rules.clone();
            }
            allow
        }
        ApprovalDecision::Deny { reason } => json!({
            "behavior": "deny",
            "message": reason.clone().unwrap_or_else(|| "the user refused".to_owned()),
        }),
        ApprovalDecision::Abort => json!({
            "behavior": "deny",
            "message": "the user stopped the turn",
        }),
    };
    StreamInput::Known(Box::new(KnownStreamInput::ControlResponse {
        response: json!({
            "subtype": "success",
            "request_id": id.0,
            "response": outcome,
        }),
        extra: Extra::new(),
    }))
}
