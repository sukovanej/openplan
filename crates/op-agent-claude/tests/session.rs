mod common;

use op_agent::{AgentEvent, TurnStop};
use op_claude::{KnownStreamInput, StreamInput};

const SESSION: &str = include_str!("fixtures/session.jsonl");

fn replay() -> common::Replay {
    common::replay("session.jsonl", SESSION)
}

#[test]
fn sends_the_prompt_as_a_user_message() {
    let replay = replay();
    let [StreamInput::Known(input)] = replay.effects.inputs.as_slice() else {
        panic!("one prompt is one line on stdin");
    };
    let KnownStreamInput::User { message, .. } = input.as_ref() else {
        panic!("the prompt is a user message");
    };
    assert_eq!(message.content.blocks().len(), 1);
}

#[test]
fn reports_the_session_once() {
    let ready = replay().all(|event| match event {
        AgentEvent::Ready(info) => Some(info.clone()),
        _ => None,
    });
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0].tools, ["Bash"]);
    assert_eq!(ready[0].model.as_deref(), Some("claude-haiku-4-5-20251001"));
    assert!(!ready[0].session.0.is_empty());
}

#[test]
fn streams_the_answer_as_deltas_and_a_whole_message() {
    let replay = replay();
    let deltas: String = replay
        .all(|event| match event {
            AgentEvent::Message { delta, .. } => Some(delta.clone()),
            _ => None,
        })
        .concat();
    let whole: String = replay
        .all(|event| match event {
            AgentEvent::MessageEnded { text, .. } => Some(text.clone()),
            _ => None,
        })
        .concat();
    assert!(!deltas.is_empty());
    assert_eq!(deltas, whole);
}

#[test]
fn pairs_a_tool_call_with_its_result() {
    let replay = replay();
    let call = replay
        .find(|event| match event {
            AgentEvent::ToolStarted(call) => Some(call.clone()),
            _ => None,
        })
        .expect("the fixture runs one tool");
    let outcome = replay
        .find(|event| match event {
            AgentEvent::ToolEnded(outcome) => Some(outcome.clone()),
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
    let replay = replay();
    let usage = replay
        .find(|event| match event {
            AgentEvent::UsageUpdated { session, .. } => Some(*session),
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
    assert_eq!(op_agent::Protocol::usage(&replay.driver), usage);
}

#[test]
fn ends_the_turn_it_started() {
    let replay = replay();
    let AgentEvent::TurnStarted {
        turn: started,
        prompt,
    } = &replay.events()[0]
    else {
        panic!("the prompt opens a turn");
    };
    assert_eq!(prompt, "the prompt");
    let ended = replay
        .find(|event| match event {
            AgentEvent::TurnEnded { turn, stop } => Some((turn.clone(), stop.clone())),
            _ => None,
        })
        .expect("the result record ends the turn");
    assert_eq!(started, &ended.0);
    assert_eq!(ended.1, TurnStop::Completed);
}
