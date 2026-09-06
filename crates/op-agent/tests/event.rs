use op_agent::{AgentEvent, ApprovalId, ItemId, Status, ToolCall, Transcript, TurnStop};
use serde_json::json;

// The events cross into the web client as JSON, so their shape is part of the interface.
#[test]
fn tags_every_event_with_its_name() {
    let event = AgentEvent::ToolStarted(ToolCall {
        item: ItemId("x".to_owned()),
        name: "Bash".to_owned(),
        input: json!({ "command": "ls" }),
    });
    assert_eq!(
        serde_json::to_value(&event).unwrap(),
        json!({ "event": "tool_started", "item": "x", "name": "Bash", "input": { "command": "ls" } })
    );
    assert_eq!(
        serde_json::to_value(AgentEvent::ApprovalResolved {
            id: ApprovalId("r".to_owned())
        })
        .unwrap(),
        json!({ "event": "approval_resolved", "id": "r" })
    );
    assert_eq!(
        serde_json::to_value(AgentEvent::TurnEnded {
            turn: op_agent::TurnId("1".to_owned()),
            stop: TurnStop::Interrupted,
        })
        .unwrap(),
        json!({ "event": "turn_ended", "turn": "1", "stop": "interrupted" })
    );
}

#[test]
fn an_event_reads_back_as_itself() {
    let events = [
        AgentEvent::ResultReady {
            value: json!({ "title": "a" }),
        },
        AgentEvent::Exited { code: None },
        AgentEvent::Failed {
            message: "x".to_owned(),
            retrying: false,
        },
    ];
    for event in events {
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(serde_json::from_str::<AgentEvent>(&json).unwrap(), event);
    }
}

#[test]
fn a_transcript_reads_back_as_itself() {
    let mut transcript = Transcript::default();
    transcript.apply(&AgentEvent::Exited { code: Some(2) });
    let json = serde_json::to_value(&transcript).unwrap();
    assert_eq!(json["status"], json!({ "kind": "exited", "code": 2 }));
    assert_eq!(
        serde_json::from_value::<Transcript>(json).unwrap().status,
        Status::Exited { code: Some(2) }
    );
}
