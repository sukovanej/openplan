use op_agent::{AgentEvent, TurnStop};
use op_agent_claude::Translator;
use op_claude::StreamOutput;

const APPROVAL: &str = include_str!("fixtures/approval.jsonl");

fn replay() -> Vec<AgentEvent> {
    let mut translator = Translator::new();
    let mut events = vec![translator.turn_started()];
    for line in op_claude::parse_jsonl::<StreamOutput>("approval.jsonl", APPROVAL) {
        events.extend(translator.translate(&line.expect("the fixture parses")));
    }
    events
}

#[test]
fn reads_the_tool_the_agent_wants_to_use() {
    let events = replay();
    let request = events
        .iter()
        .find_map(|event| match event {
            AgentEvent::ApprovalRequested(request) => Some(request),
            _ => None,
        })
        .expect("the fixture asks for one approval");
    assert_eq!(request.tool, "Write");
    assert_eq!(request.input["file_path"], "/tmp/opagent-probe.txt");
    assert_eq!(request.item.as_ref().map(|item| item.0.as_str()), {
        let call = events
            .iter()
            .find_map(|event| match event {
                AgentEvent::ToolStarted(call) => Some(call),
                _ => None,
            })
            .expect("the same tool call appears on the stream");
        Some(call.item.0.as_str())
    });
}

#[test]
fn finishes_the_turn_after_the_approval() {
    let ended = replay()
        .into_iter()
        .find_map(|event| match event {
            AgentEvent::TurnEnded { stop, .. } => Some(stop),
            _ => None,
        })
        .expect("the fixture closes the turn");
    assert_eq!(ended, TurnStop::Completed);
}
