use op_agent::{AgentEvent, TurnStop};
use op_agent_codex::{Translator, protocol::Incoming};

const SESSION: &str = include_str!("fixtures/session.jsonl");

fn messages() -> Vec<Incoming> {
    SESSION
        .lines()
        .filter(|line| !line.trim().is_empty())
        .zip(1..)
        .map(|(line, source)| {
            serde_json::from_str(line).unwrap_or_else(|error| panic!("line {source}: {error}"))
        })
        .collect()
}

fn replay() -> Vec<AgentEvent> {
    let mut translator = Translator::new("/tmp".into(), Some("gpt".to_owned()), false);
    messages()
        .iter()
        .filter_map(|message| match message {
            Incoming::Notification(notification) => Some(translator.translate(notification)),
            _ => None,
        })
        .flatten()
        .collect()
}

#[test]
fn tells_a_reply_from_a_notification() {
    let messages = messages();
    let replies = messages
        .iter()
        .filter(|message| matches!(message, Incoming::Result { .. }))
        .count();
    let notifications = messages
        .iter()
        .filter(|message| matches!(message, Incoming::Notification(_)))
        .count();
    assert_eq!(replies, 3);
    assert!(notifications > 0);
    assert!(
        !messages
            .iter()
            .any(|message| matches!(message, Incoming::Request { .. }))
    );
}

#[test]
fn keeps_the_thread_and_the_turn() {
    let mut translator = Translator::new("/tmp".into(), None, false);
    assert_eq!(translator.thread(), None);
    for message in messages() {
        if let Incoming::Notification(notification) = message {
            translator.translate(&notification);
        }
    }
    assert!(translator.thread().is_some());
    assert_eq!(translator.turn(), None);
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
fn pairs_a_shell_call_with_its_output() {
    let events = replay();
    let call = events
        .iter()
        .find_map(|event| match event {
            AgentEvent::ToolStarted(call) => Some(call),
            _ => None,
        })
        .expect("the fixture runs one command");
    let outcome = events
        .iter()
        .find_map(|event| match event {
            AgentEvent::ToolEnded(outcome) => Some(outcome),
            _ => None,
        })
        .expect("the fixture reports the command output");
    assert_eq!(call.name, "shell");
    assert_eq!(call.item, outcome.item);
    assert!(outcome.output.contains("hello"));
    assert!(!outcome.failed);
}

#[test]
fn reads_cached_tokens_as_part_of_the_input() {
    let usage = replay()
        .into_iter()
        .filter_map(|event| match event {
            AgentEvent::UsageUpdated { session, .. } => Some(session),
            _ => None,
        })
        .next_back()
        .expect("the thread reports its token usage");
    assert!(usage.cached_input_tokens > 0);
    assert!(usage.input_tokens >= usage.cached_input_tokens);
    assert_eq!(usage.cost_usd, None);
}

#[test]
fn ends_the_turn_it_started() {
    let events = replay();
    let started = events
        .iter()
        .find_map(|event| match event {
            AgentEvent::TurnStarted { turn } => Some(turn),
            _ => None,
        })
        .expect("the fixture opens a turn");
    let ended = events
        .iter()
        .find_map(|event| match event {
            AgentEvent::TurnEnded { turn, stop } => Some((turn, stop)),
            _ => None,
        })
        .expect("the fixture closes the turn");
    assert_eq!(started, ended.0);
    assert_eq!(ended.1, &TurnStop::Completed);
}
