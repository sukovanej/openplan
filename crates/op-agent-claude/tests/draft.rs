mod common;

use op_agent::{AgentEvent, Status, Transcript};
use serde_json::Value;

const DRAFT: &str = include_str!("fixtures/draft.jsonl");

fn replay() -> common::Replay {
    common::replay("draft.jsonl", DRAFT)
}

fn streamed(replay: &common::Replay) -> String {
    replay
        .all(|event| match event {
            AgentEvent::ResultDelta { delta } => Some(delta.clone()),
            _ => None,
        })
        .concat()
}

fn ready(replay: &common::Replay) -> Value {
    replay
        .find(|event| match event {
            AgentEvent::ResultReady { value } => Some(value.clone()),
            _ => None,
        })
        .expect("the turn ends with the value the schema asked for")
}

#[test]
fn streams_the_value_it_ends_with() {
    let replay = replay();
    let streamed: Value =
        serde_json::from_str(&streamed(&replay)).expect("the deltas are one JSON");
    assert_eq!(streamed, ready(&replay));
}

#[test]
fn reads_the_fields_the_schema_asked_for() {
    let value = ready(&replay());
    assert!(!value["title"].as_str().expect("a title").is_empty());
    assert!(value["body"].as_str().expect("a body").contains("mail"));
    assert!(!value["tags"].as_array().expect("tags").is_empty());
}

#[test]
fn keeps_the_built_in_tool_off_the_stream() {
    let calls = replay().all(|event| match event {
        AgentEvent::ToolStarted(call) => Some(call.name.clone()),
        _ => None,
    });
    assert!(calls.is_empty(), "{calls:?}");
}

#[test]
fn a_transcript_holds_the_draft_when_the_turn_ends() {
    let replay = replay();
    let mut transcript = Transcript::default();
    for event in replay.events() {
        transcript.apply(event);
    }
    assert_eq!(transcript.status, Status::Idle);
    assert_eq!(transcript.result, Some(ready(&replay)));
    assert_eq!(transcript.result_text, streamed(&replay));
    assert!(transcript.approvals.is_empty());
    assert!(transcript.usage.output_tokens > 0);
}
