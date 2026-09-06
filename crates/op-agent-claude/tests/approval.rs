mod common;

use op_agent::{AgentEvent, ApprovalDecision, Effects, Protocol, TurnStop};
use op_claude::{KnownStreamInput, StreamInput};

const APPROVAL: &str = include_str!("fixtures/approval.jsonl");

fn replay() -> common::Replay {
    common::replay("approval.jsonl", APPROVAL)
}

fn request(replay: &common::Replay) -> op_agent::ApprovalRequest {
    replay
        .find(|event| match event {
            AgentEvent::ApprovalRequested(request) => Some(request.clone()),
            _ => None,
        })
        .expect("the fixture asks for one approval")
}

#[test]
fn reads_the_tool_the_agent_wants_to_use() {
    let replay = replay();
    let request = request(&replay);
    assert_eq!(request.tool, "Write");
    assert_eq!(request.input["file_path"], "/tmp/opagent-probe.txt");
    let call = replay
        .find(|event| match event {
            AgentEvent::ToolStarted(call) => Some(call.clone()),
            _ => None,
        })
        .expect("the same tool call appears on the stream");
    assert_eq!(request.item, Some(call.item));
}

#[test]
fn finishes_the_turn_after_the_approval() {
    let ended = replay()
        .find(|event| match event {
            AgentEvent::TurnEnded { stop, .. } => Some(stop.clone()),
            _ => None,
        })
        .expect("the fixture closes the turn");
    assert_eq!(ended, TurnStop::Completed);
}

// The CLI runs the tool with the input the decision echoes back, so a decision without it would
// run the tool with nothing.
#[test]
fn echoes_the_tool_input_back_with_the_decision() {
    let mut replay = replay();
    let request = request(&replay);
    let mut effects = Effects::default();
    replay
        .driver
        .approve(request.id.clone(), ApprovalDecision::Allow, &mut effects);

    let [StreamInput::Known(input)] = effects.inputs.as_slice() else {
        panic!("a decision is one line on stdin");
    };
    let KnownStreamInput::ControlResponse { response, .. } = input.as_ref() else {
        panic!("a decision is a control response");
    };
    assert_eq!(response["request_id"], request.id.0);
    assert_eq!(response["response"]["behavior"], "allow");
    assert_eq!(response["response"]["updatedInput"], request.input);
    assert_eq!(
        effects.events,
        [AgentEvent::ApprovalResolved { id: request.id }]
    );
}

#[test]
fn stops_the_turn_when_the_user_aborts() {
    let mut replay = replay();
    let request = request(&replay);
    let mut effects = Effects::default();
    replay
        .driver
        .approve(request.id, ApprovalDecision::Abort, &mut effects);

    let kinds: Vec<_> = effects
        .inputs
        .iter()
        .map(|input| match input {
            StreamInput::Known(known) => match known.as_ref() {
                KnownStreamInput::ControlResponse { response, .. } => {
                    response["response"]["behavior"]
                        .as_str()
                        .unwrap()
                        .to_owned()
                }
                KnownStreamInput::ControlRequest { request, .. } => {
                    request["subtype"].as_str().unwrap().to_owned()
                }
                _ => "other".to_owned(),
            },
            StreamInput::Unknown(_) => "unknown".to_owned(),
        })
        .collect();
    assert_eq!(kinds, ["deny", "interrupt"]);
}
