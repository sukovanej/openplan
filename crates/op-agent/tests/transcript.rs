use op_agent::{
    AgentEvent, ApprovalId, ApprovalRequest, Entry, ItemId, SessionId, SessionInfo, Status,
    ToolCall, ToolOutcome, Transcript, TurnId, TurnStop, Usage,
};
use serde_json::json;

fn item(id: &str) -> ItemId {
    ItemId(id.to_owned())
}

fn turn() -> TurnId {
    TurnId("1".to_owned())
}

fn fold(events: &[AgentEvent]) -> Transcript {
    let mut transcript = Transcript::default();
    for event in events {
        transcript.apply(event);
    }
    transcript
}

#[test]
fn starts_idle_once_the_agent_is_ready() {
    let mut transcript = Transcript::default();
    assert_eq!(transcript.status, Status::Starting);
    transcript.apply(&AgentEvent::Ready(SessionInfo {
        session: SessionId("s".to_owned()),
        cwd: "/tmp".into(),
        model: None,
        tools: Vec::new(),
    }));
    assert_eq!(transcript.status, Status::Idle);
    assert_eq!(transcript.info.as_ref().unwrap().session.0, "s");
}

#[test]
fn folds_deltas_into_one_entry_and_the_whole_text_over_them() {
    let transcript = fold(&[
        AgentEvent::TurnStarted {
            turn: turn(),
            prompt: "hi".to_owned(),
        },
        AgentEvent::Thinking {
            item: item("t"),
            delta: "hm".to_owned(),
        },
        AgentEvent::Message {
            item: item("m"),
            delta: "hel".to_owned(),
        },
        AgentEvent::Message {
            item: item("m"),
            delta: "lo".to_owned(),
        },
        AgentEvent::MessageEnded {
            item: item("m"),
            text: "hello".to_owned(),
        },
    ]);
    assert!(transcript.running());
    assert_eq!(
        transcript.entries,
        [
            Entry::Prompt {
                text: "hi".to_owned()
            },
            Entry::Thinking {
                item: item("t"),
                text: "hm".to_owned(),
                done: false,
            },
            Entry::Message {
                item: item("m"),
                text: "hello".to_owned(),
                done: true,
            },
        ]
    );
}

#[test]
fn keeps_streamed_tool_output_when_the_end_reports_none() {
    let call = ToolCall {
        item: item("x"),
        name: "shell".to_owned(),
        input: json!({ "command": "ls" }),
    };
    let transcript = fold(&[
        AgentEvent::ToolStarted(call.clone()),
        AgentEvent::ToolOutput {
            item: item("x"),
            delta: "a\n".to_owned(),
        },
        AgentEvent::ToolEnded(ToolOutcome {
            item: item("x"),
            output: String::new(),
            failed: true,
        }),
    ]);
    assert_eq!(
        transcript.entries,
        [Entry::Tool {
            item: item("x"),
            name: "shell".to_owned(),
            input: call.input,
            output: "a\n".to_owned(),
            done: true,
            failed: true,
        }]
    );
}

#[test]
fn holds_an_approval_until_it_is_resolved_or_the_turn_ends() {
    let request = ApprovalRequest {
        id: ApprovalId("r".to_owned()),
        item: None,
        tool: "Write".to_owned(),
        input: json!({}),
        reason: None,
    };
    let mut transcript = fold(&[AgentEvent::ApprovalRequested(request.clone())]);
    assert_eq!(transcript.approvals, std::slice::from_ref(&request));
    transcript.apply(&AgentEvent::ApprovalResolved {
        id: request.id.clone(),
    });
    assert!(transcript.approvals.is_empty());

    transcript.apply(&AgentEvent::ApprovalRequested(request));
    transcript.apply(&AgentEvent::TurnEnded {
        turn: turn(),
        stop: TurnStop::Interrupted,
    });
    assert!(transcript.approvals.is_empty());
    assert_eq!(transcript.status, Status::Idle);
}

#[test]
fn keeps_the_last_result_while_the_next_one_streams() {
    let mut transcript = fold(&[
        AgentEvent::TurnStarted {
            turn: turn(),
            prompt: "draft".to_owned(),
        },
        AgentEvent::ResultDelta {
            delta: "{\"title\":".to_owned(),
        },
        AgentEvent::ResultDelta {
            delta: "\"a\"}".to_owned(),
        },
        AgentEvent::ResultReady {
            value: json!({ "title": "a" }),
        },
        AgentEvent::TurnEnded {
            turn: turn(),
            stop: TurnStop::Completed,
        },
        AgentEvent::TurnStarted {
            turn: TurnId("2".to_owned()),
            prompt: "again".to_owned(),
        },
        AgentEvent::ResultDelta {
            delta: "{\"ti".to_owned(),
        },
    ]);
    assert_eq!(transcript.result, Some(json!({ "title": "a" })));
    assert_eq!(transcript.result_text, "{\"ti");
    assert_eq!(transcript.turn, Some(TurnId("2".to_owned())));

    transcript.apply(&AgentEvent::Exited { code: Some(1) });
    assert_eq!(transcript.status, Status::Exited { code: Some(1) });
    assert_eq!(transcript.turn, None);
}

#[test]
fn tracks_usage_and_the_budget() {
    let usage = Usage {
        input_tokens: 10,
        ..Usage::default()
    };
    let transcript = fold(&[
        AgentEvent::UsageUpdated {
            turn: usage,
            session: usage,
            context_window: Some(200_000),
        },
        AgentEvent::BudgetExhausted { session: usage },
        AgentEvent::Failed {
            message: "no".to_owned(),
            retrying: true,
        },
    ]);
    assert_eq!(transcript.usage, usage);
    assert_eq!(transcript.context_window, Some(200_000));
    assert!(transcript.over_budget);
    assert_eq!(
        transcript.entries,
        [Entry::Failure {
            message: "no".to_owned(),
            retrying: true,
        }]
    );
}
