mod common;

use op_agent::{AgentEvent, ApprovalDecision, Effects, Protocol};
use op_agent_codex::{
    protocol::{Incoming, Outgoing},
    translate::decision,
};

const APPROVAL: &str = include_str!("fixtures/approval.jsonl");

fn request(replay: &common::Replay) -> op_agent::ApprovalRequest {
    replay
        .find(|event| match event {
            AgentEvent::ApprovalRequested(request) => Some(request.clone()),
            _ => None,
        })
        .expect("the fixture asks for one approval")
}

#[test]
fn reads_a_command_the_agent_wants_to_run() {
    let replay = common::replay(APPROVAL, common::options());
    let request = request(&replay);
    assert_eq!(request.tool, "shell");
    assert_eq!(request.input["command"], "/bin/zsh -lc whoami");
    assert_eq!(request.input["cwd"], "/tmp");
    assert!(request.item.is_some());
    assert_eq!(request.id.0, "0");
}

// The fixture was recorded with the decision given, so the server's own `serverRequest/resolved`
// follows; a decision the driver already reported must not be reported twice.
#[test]
fn reports_a_request_the_server_resolved_once() {
    let mut driver = op_agent_codex::Driver::new(common::options());
    let mut effects = Effects::default();
    driver.open(&mut effects);
    driver.prompt("the prompt".to_owned(), &mut effects);
    let mut resolved = 0;
    for message in common::messages(APPROVAL) {
        let asked = matches!(message, Incoming::Request { .. });
        driver.read(message, &mut effects);
        if asked {
            let request = effects
                .events
                .iter()
                .find_map(|event| match event {
                    AgentEvent::ApprovalRequested(request) => Some(request.clone()),
                    _ => None,
                })
                .expect("the request reached the caller");
            driver.approve(request.id, ApprovalDecision::Allow, &mut effects);
            let Some(Outgoing::Response { result, .. }) = effects.inputs.last() else {
                panic!("the decision goes back as a response");
            };
            assert_eq!(result["decision"], "accept");
        }
        resolved = effects
            .events
            .iter()
            .filter(|event| matches!(event, AgentEvent::ApprovalResolved { .. }))
            .count();
    }
    assert_eq!(resolved, 1);
}

#[test]
fn drops_a_request_the_server_resolved_on_its_own() {
    let mut replay = common::replay(APPROVAL, common::options());
    let request = request(&replay);
    let resolved = replay.all(|event| match event {
        AgentEvent::ApprovalResolved { id } => Some(id.clone()),
        _ => None,
    });
    assert_eq!(resolved, std::slice::from_ref(&request.id));

    let mut effects = Effects::default();
    replay
        .driver
        .approve(request.id, ApprovalDecision::Allow, &mut effects);
    assert!(effects.inputs.is_empty());
    assert!(effects.events.is_empty());
}

#[test]
fn sends_a_decision_the_prompt_offers() {
    let offered = ["accept".to_owned(), "cancel".to_owned()];
    assert_eq!(decision(&ApprovalDecision::Allow, &offered), "accept");
    assert_eq!(
        decision(&ApprovalDecision::AllowForSession, &offered),
        "accept"
    );
    assert_eq!(
        decision(&ApprovalDecision::Deny { reason: None }, &offered),
        "cancel"
    );
    assert_eq!(decision(&ApprovalDecision::Abort, &offered), "cancel");
}

#[test]
fn keeps_the_decision_when_the_prompt_offers_it() {
    let offered = [
        "accept".to_owned(),
        "acceptForSession".to_owned(),
        "decline".to_owned(),
        "cancel".to_owned(),
    ];
    assert_eq!(
        decision(&ApprovalDecision::AllowForSession, &offered),
        "acceptForSession"
    );
    assert_eq!(
        decision(&ApprovalDecision::Deny { reason: None }, &offered),
        "decline"
    );
}
