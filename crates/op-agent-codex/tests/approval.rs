use op_agent::ApprovalDecision;
use op_agent_codex::{
    protocol::Incoming,
    translate::{approval_request, decision},
};

const APPROVAL: &str = include_str!("fixtures/approval.jsonl");

#[test]
fn reads_a_command_the_agent_wants_to_run() {
    let request = APPROVAL
        .lines()
        .filter_map(|line| serde_json::from_str::<Incoming>(line).ok())
        .find_map(|message| match message {
            Incoming::Request { id, method, params } => approval_request(&id, &method, &params),
            _ => None,
        })
        .expect("the fixture asks for one approval");
    assert_eq!(request.tool, "shell");
    assert_eq!(request.input["command"], "/bin/zsh -lc whoami");
    assert_eq!(request.input["cwd"], "/tmp");
    assert!(request.item.is_some());
    assert_eq!(request.id.0, "0");
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
