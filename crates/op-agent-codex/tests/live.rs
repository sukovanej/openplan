use op_agent::{
    Agent, AgentEvent, ApprovalDecision, McpPolicy, Permissions, Persistence, SessionOptions,
    TurnStop,
};
use op_agent_codex::Codex;

// Needs the `codex` binary and a signed-in account, so it is opt-in:
// cargo test -p op-agent-codex -- --ignored
#[tokio::test]
#[ignore]
async fn answers_two_turns_on_one_thread() {
    let options = SessionOptions::new(std::env::temp_dir())
        .mcp(McpPolicy::Disabled)
        .persistence(Persistence::Ephemeral);
    let mut session = Codex::new().start(options).expect("codex is on the path");

    session
        .handle()
        .prompt("Reply with the single word: ONE")
        .await
        .expect("the session takes a prompt");
    let first = drain_turn(&mut session).await;
    assert_eq!(first.stop, TurnStop::Completed);
    assert!(first.text.contains("ONE"), "{}", first.text);

    session
        .handle()
        .prompt("Reply with the single word: TWO")
        .await
        .expect("the session takes a second prompt");
    let second = drain_turn(&mut session).await;
    assert_eq!(second.stop, TurnStop::Completed);
    assert!(second.text.contains("TWO"), "{}", second.text);

    // The point of one long-lived thread: the second turn reads the first turn's cache instead of
    // paying for the prompt again.
    assert!(second.cached_input_tokens > 0);
    session.handle().shutdown().await.ok();
}

struct Turn {
    text: String,
    stop: TurnStop,
    cached_input_tokens: u64,
}

async fn drain_turn(session: &mut op_agent::Session) -> Turn {
    let mut turn = Turn {
        text: String::new(),
        stop: TurnStop::Failed,
        cached_input_tokens: 0,
    };
    while let Some(event) = session.next_event().await {
        match event {
            AgentEvent::MessageEnded { text, .. } => turn.text.push_str(&text),
            AgentEvent::UsageUpdated { turn: usage, .. } => {
                turn.cached_input_tokens = usage.cached_input_tokens;
            }
            AgentEvent::TurnEnded { stop, .. } => {
                turn.stop = stop;
                return turn;
            }
            AgentEvent::Exited { code } => panic!("the agent left early with {code:?}"),
            _ => {}
        }
    }
    panic!("the event stream ended before the turn did");
}

#[tokio::test]
#[ignore]
async fn asks_before_it_runs_a_command() {
    let options = SessionOptions::new(std::env::temp_dir())
        .permissions(Permissions::Ask)
        .mcp(McpPolicy::Disabled)
        .persistence(Persistence::Ephemeral);
    let mut session = Codex::new().start(options).expect("codex is on the path");
    session
        .handle()
        .prompt("Run the shell command `whoami` and tell me its output.")
        .await
        .expect("the session takes a prompt");

    let mut asked = false;
    let mut output = String::new();
    while let Some(event) = session.next_event().await {
        match event {
            AgentEvent::ApprovalRequested(request) => {
                asked = true;
                assert_eq!(request.tool, "shell");
                session
                    .handle()
                    .approve(request.id, ApprovalDecision::Allow)
                    .await
                    .expect("the session takes a decision");
            }
            AgentEvent::ToolEnded(outcome) => output.push_str(&outcome.output),
            AgentEvent::TurnEnded { .. } => break,
            AgentEvent::Exited { code } => panic!("the agent left early with {code:?}"),
            _ => {}
        }
    }
    assert!(asked, "the agent ran the command without asking");
    assert!(!output.trim().is_empty(), "the command produced no output");
    session.handle().shutdown().await.ok();
}

#[tokio::test]
#[ignore]
async fn stops_a_turn_it_is_told_to_stop() {
    let options = SessionOptions::new(std::env::temp_dir())
        .mcp(McpPolicy::Disabled)
        .persistence(Persistence::Ephemeral);
    let mut session = Codex::new().start(options).expect("codex is on the path");
    session
        .handle()
        .prompt("Count from 1 to 2000, one number per line. No other words.")
        .await
        .expect("the session takes a prompt");

    let mut stopped = false;
    while let Some(event) = session.next_event().await {
        match event {
            AgentEvent::Message { .. } if !stopped => {
                stopped = true;
                session
                    .handle()
                    .interrupt()
                    .await
                    .expect("the session takes an interrupt");
            }
            AgentEvent::TurnEnded { stop, .. } => {
                assert!(stopped, "the turn ended before the interrupt");
                assert_eq!(stop, TurnStop::Interrupted);
                session.handle().shutdown().await.ok();
                return;
            }
            AgentEvent::Exited { code } => panic!("the agent left early with {code:?}"),
            _ => {}
        }
    }
    panic!("the event stream ended before the turn did");
}
