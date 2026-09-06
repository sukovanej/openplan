mod common;

use op_agent::{AgentEvent, Effects, Protocol, Status, Transcript};
use op_agent_codex::{Driver, protocol::Incoming};
use serde_json::{Value, json};

const DRAFT: &str = include_str!("fixtures/draft.jsonl");

fn schema() -> Value {
    json!({ "type": "object" })
}

fn replay() -> common::Replay {
    common::replay(DRAFT, common::options().schema(schema()))
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
fn sends_the_schema_with_the_turn() {
    let replay = replay();
    let turn = replay
        .effects
        .inputs
        .iter()
        .find_map(|input| match input {
            op_agent_codex::protocol::Outgoing::Request { method, params, .. }
                if *method == "turn/start" =>
            {
                Some(params.clone())
            }
            _ => None,
        })
        .expect("the prompt starts a turn");
    assert_eq!(turn["outputSchema"], schema());
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

// The output schema shapes the notes the agent writes while it works as well as its answer, so a
// note that reached the draft would overwrite it with progress text.
#[test]
fn keeps_a_note_out_of_the_draft() {
    let mut driver = Driver::new(common::options().schema(schema()));
    let mut effects = Effects::default();
    for line in [
        json!({
            "method": "item/started",
            "params": { "threadId": "t", "turnId": "u", "startedAtMs": 0,
                "item": { "type": "agentMessage", "id": "note", "text": "", "phase": "commentary" } }
        }),
        json!({
            "method": "item/agentMessage/delta",
            "params": { "threadId": "t", "turnId": "u", "itemId": "note", "delta": "working on it" }
        }),
    ] {
        let message: Incoming = serde_json::from_value(line).expect("the line parses");
        driver.read(message, &mut effects);
    }
    assert_eq!(
        effects.events,
        [AgentEvent::Message {
            item: op_agent::ItemId("note".to_owned()),
            delta: "working on it".to_owned(),
        }]
    );
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
    assert!(
        transcript
            .entries
            .iter()
            .any(|entry| matches!(entry, op_agent::Entry::Prompt { text } if text == "the prompt"))
    );
}
