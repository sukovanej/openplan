mod common;

use op_agent::{AgentEvent, Protocol, TurnStop};
use op_agent_codex::protocol::{Incoming, Outgoing};

const SESSION: &str = include_str!("fixtures/session.jsonl");

fn replay() -> common::Replay {
    common::replay(SESSION, common::options())
}

#[test]
fn tells_a_reply_from_a_notification() {
    let messages = common::messages(SESSION);
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

// The prompt arrives before the thread exists, so it waits for the handshake and goes out as the
// first turn once the thread has an id.
#[test]
fn shakes_hands_before_it_sends_the_prompt() {
    let replay = replay();
    assert_eq!(
        replay.methods(),
        ["initialize", "initialized", "thread/start", "turn/start"]
    );
    let Outgoing::Request { params, .. } = &replay.effects.inputs[3] else {
        panic!("the turn is a request");
    };
    assert_eq!(params["input"][0]["text"], "the prompt");
    assert_eq!(params["threadId"], replay.driver.thread().unwrap());
}

#[test]
fn keeps_the_thread_and_the_turn() {
    let replay = replay();
    assert!(replay.driver.thread().is_some());
    assert_eq!(replay.driver.turn(), None);
    let ready = replay.all(|event| match event {
        AgentEvent::Ready(info) => Some(info.clone()),
        _ => None,
    });
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0].session.0, replay.driver.thread().unwrap());
    assert_eq!(ready[0].model.as_deref(), Some("gpt"));
}

#[test]
fn streams_the_answer_as_deltas_and_a_whole_message() {
    let replay = replay();
    let deltas = replay
        .all(|event| match event {
            AgentEvent::Message { delta, .. } => Some(delta.clone()),
            _ => None,
        })
        .concat();
    let whole = replay
        .all(|event| match event {
            AgentEvent::MessageEnded { text, .. } => Some(text.clone()),
            _ => None,
        })
        .concat();
    assert!(!deltas.is_empty());
    assert_eq!(deltas, whole);
}

#[test]
fn pairs_a_shell_call_with_its_output() {
    let replay = replay();
    let call = replay
        .find(|event| match event {
            AgentEvent::ToolStarted(call) => Some(call.clone()),
            _ => None,
        })
        .expect("the fixture runs one command");
    let outcome = replay
        .find(|event| match event {
            AgentEvent::ToolEnded(outcome) => Some(outcome.clone()),
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
    let replay = replay();
    let usage = replay
        .all(|event| match event {
            AgentEvent::UsageUpdated { session, .. } => Some(*session),
            _ => None,
        })
        .pop()
        .expect("the thread reports its token usage");
    assert!(usage.cached_input_tokens > 0);
    assert!(usage.input_tokens >= usage.cached_input_tokens);
    assert_eq!(usage.cost_usd, None);
    assert_eq!(replay.driver.usage(), usage);
}

#[test]
fn ends_the_turn_it_started() {
    let replay = replay();
    let started = replay
        .find(|event| match event {
            AgentEvent::TurnStarted { turn, prompt } => Some((turn.clone(), prompt.clone())),
            _ => None,
        })
        .expect("the fixture opens a turn");
    let ended = replay
        .find(|event| match event {
            AgentEvent::TurnEnded { turn, stop } => Some((turn.clone(), stop.clone())),
            _ => None,
        })
        .expect("the fixture closes the turn");
    assert_eq!(started.1, "the prompt");
    assert_eq!(started.0, ended.0);
    assert_eq!(ended.1, TurnStop::Completed);
}

// The server reports a fatal error as a notification and again on the turn it ended; the caller
// should read it once.
#[test]
fn reports_a_fatal_error_once() {
    let mut driver = op_agent_codex::Driver::new(common::options());
    let mut effects = op_agent::Effects::default();
    for line in [
        serde_json::json!({
            "method": "turn/started",
            "params": { "threadId": "t", "turn": { "id": "u", "status": "inProgress" } }
        }),
        serde_json::json!({
            "method": "error",
            "params": { "error": { "message": "usage limit" }, "willRetry": false }
        }),
        serde_json::json!({
            "method": "turn/completed",
            "params": { "threadId": "t", "turn": { "id": "u", "status": "failed",
                "error": { "message": "usage limit" } } }
        }),
    ] {
        let message: Incoming = serde_json::from_value(line).expect("the line parses");
        driver.read(message, &mut effects);
    }
    let failures = effects
        .events
        .iter()
        .filter(|event| matches!(event, AgentEvent::Failed { .. }))
        .count();
    assert_eq!(failures, 1);
    assert!(matches!(
        effects.events.last(),
        Some(AgentEvent::TurnEnded {
            stop: TurnStop::Failed,
            ..
        })
    ));
}
