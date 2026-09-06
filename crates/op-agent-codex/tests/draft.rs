use op_agent::AgentEvent;
use op_agent_codex::{Translator, protocol::Incoming};
use serde_json::{Value, json};

const DRAFT: &str = include_str!("fixtures/draft.jsonl");

fn replay() -> Vec<AgentEvent> {
    let mut translator = Translator::new("/tmp".into(), None, true);
    DRAFT
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str(line).ok())
        .filter_map(|message| match message {
            Incoming::Notification(notification) => Some(translator.translate(&notification)),
            _ => None,
        })
        .flatten()
        .collect()
}

fn streamed(events: &[AgentEvent]) -> String {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ResultDelta { delta } => Some(delta.as_str()),
            _ => None,
        })
        .collect()
}

fn ready(events: &[AgentEvent]) -> &Value {
    events
        .iter()
        .find_map(|event| match event {
            AgentEvent::ResultReady { value } => Some(value),
            _ => None,
        })
        .expect("the turn ends with the value the schema asked for")
}

#[test]
fn streams_the_value_it_ends_with() {
    let events = replay();
    let streamed: Value =
        serde_json::from_str(&streamed(&events)).expect("the deltas are one JSON");
    assert_eq!(&streamed, ready(&events));
}

#[test]
fn reads_the_fields_the_schema_asked_for() {
    let events = replay();
    let value = ready(&events);
    assert!(!value["title"].as_str().expect("a title").is_empty());
    assert!(value["body"].as_str().expect("a body").contains("mail"));
    assert!(!value["tags"].as_array().expect("tags").is_empty());
}

// The output schema shapes the notes the agent writes while it works as well as its answer, so a
// note that reached the draft would overwrite it with progress text.
#[test]
fn keeps_a_note_out_of_the_draft() {
    let mut translator = Translator::new("/tmp".into(), None, true);
    let events = [
        json!({
            "method": "item/started",
            "params": { "threadId": "t", "turnId": "u", "startedAtMs": 0,
                "item": { "type": "agentMessage", "id": "note", "text": "", "phase": "commentary" } }
        }),
        json!({
            "method": "item/agentMessage/delta",
            "params": { "threadId": "t", "turnId": "u", "itemId": "note", "delta": "working on it" }
        }),
    ]
    .iter()
    .flat_map(|line| {
        let Incoming::Notification(notification) = serde_json::from_value(line.clone()).unwrap()
        else {
            panic!("a notification carries no id");
        };
        translator.translate(&notification)
    })
    .collect::<Vec<_>>();

    assert_eq!(streamed(&events), "");
    assert_eq!(
        events,
        [AgentEvent::Message {
            item: op_agent::ItemId("note".to_owned()),
            delta: "working on it".to_owned(),
        }]
    );
}
