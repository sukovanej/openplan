use op_agent::{AgentEvent, TurnStop};
use op_agent_claude::Translator;
use op_claude::StreamOutput;

const SESSION: &str = include_str!("fixtures/session.jsonl");

fn replay() -> Vec<AgentEvent> {
    let mut translator = Translator::new();
    let mut events = vec![translator.turn_started()];
    for (line, source) in op_claude::parse_jsonl::<StreamOutput>("session.jsonl", SESSION).zip(1..)
    {
        let output = line.unwrap_or_else(|error| panic!("line {source}: {error}"));
        events.extend(translator.translate(&output));
    }
    events
}

#[test]
fn reports_the_session_once() {
    let ready: Vec<_> = replay()
        .into_iter()
        .filter_map(|event| match event {
            AgentEvent::Ready(info) => Some(info),
            _ => None,
        })
        .collect();
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0].tools, ["Bash"]);
    assert_eq!(ready[0].model.as_deref(), Some("claude-haiku-4-5-20251001"));
    assert!(!ready[0].session.0.is_empty());
}

#[test]
fn streams_the_answer_as_deltas_and_a_whole_message() {
    let events = replay();
    let deltas: String = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::Message { delta, .. } => Some(delta.as_str()),
            _ => None,
        })
        .collect();
    let whole: String = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::MessageEnded { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert!(!deltas.is_empty());
    assert_eq!(deltas, whole);
}

#[test]
fn pairs_a_tool_call_with_its_result() {
    let events = replay();
    let call = events
        .iter()
        .find_map(|event| match event {
            AgentEvent::ToolStarted(call) => Some(call),
            _ => None,
        })
        .expect("the fixture runs one tool");
    let outcome = events
        .iter()
        .find_map(|event| match event {
            AgentEvent::ToolEnded(outcome) => Some(outcome),
            _ => None,
        })
        .expect("the fixture reports the tool result");
    assert_eq!(call.name, "Bash");
    assert_eq!(call.input["command"], "echo hello");
    assert_eq!(call.item, outcome.item);
    assert_eq!(outcome.output, "hello");
    assert!(!outcome.failed);
}

#[test]
fn counts_cache_reads_as_input_tokens() {
    let usage = replay()
        .into_iter()
        .find_map(|event| match event {
            AgentEvent::UsageUpdated { session, .. } => Some(session),
            _ => None,
        })
        .expect("the result record carries usage");
    assert!(usage.cached_input_tokens > 0);
    assert!(usage.input_tokens >= usage.cached_input_tokens);
    assert_eq!(
        usage.total_tokens(),
        usage.input_tokens + usage.cache_write_tokens + usage.output_tokens
    );
    assert!(usage.cost_usd.unwrap_or_default() > 0.0);
}

#[test]
fn ends_the_turn_it_started() {
    let events = replay();
    let AgentEvent::TurnStarted { turn: started } = &events[0] else {
        panic!("the replay opens a turn");
    };
    let ended = events
        .iter()
        .find_map(|event| match event {
            AgentEvent::TurnEnded { turn, stop } => Some((turn, stop)),
            _ => None,
        })
        .expect("the result record ends the turn");
    assert_eq!(started, ended.0);
    assert_eq!(ended.1, &TurnStop::Completed);
}
