use op_agent::{Budget, Usage, process};
use serde_json::{Value, json};
use tokio::process::Command;

#[tokio::test]
async fn carries_json_lines_both_ways() {
    let mut process =
        process::spawn::<Value, Value>(Command::new("cat")).expect("cat is on the path");
    process
        .outgoing
        .send(json!({ "type": "user", "text": "one" }))
        .await
        .expect("the writer accepts a line");
    process
        .outgoing
        .send(json!({ "type": "user", "text": "two" }))
        .await
        .expect("the writer accepts a line");

    let first = process.incoming.recv().await.expect("cat echoes the line");
    let second = process.incoming.recv().await.expect("cat echoes the line");
    assert_eq!(first["text"], "one");
    assert_eq!(second["text"], "two");
}

#[tokio::test]
async fn reports_a_binary_that_is_not_there() {
    let error = process::spawn::<Value, Value>(Command::new("op-agent-no-such-binary"))
        .err()
        .expect("a missing binary cannot start");
    assert!(error.to_string().contains("op-agent-no-such-binary"));
}

#[test]
fn a_budget_without_limits_never_runs_out() {
    let usage = Usage {
        input_tokens: 1_000_000,
        cost_usd: Some(500.0),
        ..Usage::default()
    };
    assert!(!Budget::default().exhausted_by(&usage));
}

#[test]
fn a_budget_runs_out_on_either_limit() {
    let spent = Usage {
        input_tokens: 900,
        output_tokens: 100,
        cost_usd: Some(0.5),
        ..Usage::default()
    };
    assert!(Budget::default().max_tokens(1000).exhausted_by(&spent));
    assert!(!Budget::default().max_tokens(1001).exhausted_by(&spent));
    assert!(Budget::default().max_cost_usd(0.5).exhausted_by(&spent));
    assert!(!Budget::default().max_cost_usd(0.51).exhausted_by(&spent));
}

#[test]
fn a_session_adds_up_its_turns() {
    let turn = Usage {
        input_tokens: 100,
        cached_input_tokens: 80,
        cache_write_tokens: 20,
        output_tokens: 10,
        reasoning_tokens: 4,
        cost_usd: Some(0.25),
    };
    let mut session = Usage::default();
    session.add(&turn);
    session.add(&turn);
    assert_eq!(session.input_tokens, 200);
    assert_eq!(session.uncached_input_tokens(), 40);
    assert_eq!(session.total_tokens(), 260);
    assert_eq!(session.cost_usd, Some(0.5));
}
