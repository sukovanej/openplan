use op_agent::AgentEvent;
use op_agent_claude::Translator;
use op_claude::StreamOutput;
use serde_json::Value;

const DRAFT: &str = include_str!("fixtures/draft.jsonl");

fn replay() -> Vec<AgentEvent> {
    let mut translator = Translator::new();
    let mut events = vec![translator.turn_started()];
    for line in op_claude::parse_jsonl::<StreamOutput>("draft.jsonl", DRAFT) {
        events.extend(translator.translate(&line.expect("the fixture parses")));
    }
    events
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

#[test]
fn keeps_the_built_in_tool_off_the_stream() {
    let calls: Vec<_> = replay()
        .into_iter()
        .filter_map(|event| match event {
            AgentEvent::ToolStarted(call) => Some(call.name),
            _ => None,
        })
        .collect();
    assert!(calls.is_empty(), "{calls:?}");
}
