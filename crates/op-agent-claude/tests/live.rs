use op_agent::{
    Agent, AgentEvent, ApprovalDecision, McpPolicy, Permissions, Persistence, SessionOptions,
    TurnStop,
};
use op_agent_claude::{ClaudeCode, Settings, Skills, Tools};

// Needs the `claude` binary and a signed-in account, so it is opt-in:
// cargo test -p op-agent-claude -- --ignored
#[tokio::test]
#[ignore]
async fn answers_two_turns_on_one_process() {
    let agent = ClaudeCode::new()
        .tools(Tools::None)
        .skills(Skills::Disabled);
    let options = SessionOptions::new(std::env::temp_dir())
        .model("claude-haiku-4-5-20251001")
        .mcp(McpPolicy::Disabled)
        .persistence(Persistence::Ephemeral);
    let mut session = agent.start(options).expect("claude is on the path");

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

    // The point of one long-lived process: the second turn reads the first turn's cache instead of
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
async fn asks_before_it_writes_a_file() {
    let workspace = std::env::temp_dir().join("op-agent-claude-live");
    std::fs::create_dir_all(&workspace).expect("the workspace can be made");
    let target = workspace.join("greeting.txt");
    std::fs::remove_file(&target).ok();

    let agent = ClaudeCode::new()
        .tools(Tools::Only(vec!["Write".to_owned()]))
        .skills(Skills::Disabled)
        .settings(Settings::Ignore);
    let options = SessionOptions::new(&workspace)
        .model("claude-haiku-4-5-20251001")
        .permissions(Permissions::Ask)
        .mcp(McpPolicy::Disabled)
        .persistence(Persistence::Ephemeral);
    let mut session = agent.start(options).expect("claude is on the path");
    session
        .handle()
        .prompt("Create the file greeting.txt holding the word hello.")
        .await
        .expect("the session takes a prompt");

    let mut asked = false;
    while let Some(event) = session.next_event().await {
        match event {
            AgentEvent::ApprovalRequested(request) => {
                asked = true;
                assert_eq!(request.tool, "Write");
                session
                    .handle()
                    .approve(request.id, ApprovalDecision::Allow)
                    .await
                    .expect("the session takes a decision");
            }
            AgentEvent::TurnEnded { .. } => break,
            AgentEvent::Exited { code } => panic!("the agent left early with {code:?}"),
            _ => {}
        }
    }
    assert!(asked, "the agent wrote the file without asking");
    assert_eq!(std::fs::read_to_string(&target).unwrap().trim(), "hello");
    session.handle().shutdown().await.ok();
}

#[tokio::test]
#[ignore]
async fn stops_a_turn_it_is_told_to_stop() {
    let agent = ClaudeCode::new()
        .tools(Tools::None)
        .skills(Skills::Disabled)
        .settings(Settings::Ignore);
    let options = SessionOptions::new(std::env::temp_dir())
        .model("claude-haiku-4-5-20251001")
        .mcp(McpPolicy::Disabled)
        .persistence(Persistence::Ephemeral);
    let mut session = agent.start(options).expect("claude is on the path");
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
