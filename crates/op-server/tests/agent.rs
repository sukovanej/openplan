mod common;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use axum::http::StatusCode;
use common::*;
use op_agent::session::Channel;
use op_agent::{
    Agent, AgentError, AgentEvent, AgentKind, ApprovalId, ApprovalRequest, Command, ItemId,
    Session, SessionOptions, TurnId, TurnStop,
};
use op_server::AppState;
use serde_json::{Value, json};
use tokio::sync::mpsc;

struct Started {
    options: SessionOptions,
    commands: mpsc::Receiver<Command>,
    events: mpsc::Sender<AgentEvent>,
}

// One agent whose `start` hands the test the other end of the session: the commands the daemon
// sends, and the event sender the test speaks through.
struct Fake {
    kind: AgentKind,
    started: Mutex<std::sync::mpsc::Sender<Started>>,
}

impl Agent for Fake {
    fn kind(&self) -> AgentKind {
        self.kind
    }

    fn start(&self, options: SessionOptions) -> Result<Session, AgentError> {
        let (session, Channel { commands, events }) = op_agent::session::open();
        self.started
            .lock()
            .expect("fake agent lock poisoned")
            .send(Started {
                options,
                commands,
                events,
            })
            .expect("the test still holds the receiver");
        Ok(session)
    }
}

fn with_agent() -> (
    tempfile::TempDir,
    AppState,
    std::sync::mpsc::Receiver<Started>,
) {
    let (dir, state) = local_state();
    let (tx, rx) = std::sync::mpsc::channel();
    let fake = Arc::new(Fake {
        kind: AgentKind::ClaudeCode,
        started: Mutex::new(tx),
    });
    let state = state.with_agents(BTreeMap::from([(
        AgentKind::ClaudeCode,
        fake as Arc<dyn Agent>,
    )]));
    state.start_projects();
    (dir, state, rx)
}

async fn start_session(state: &AppState, body: Value) -> String {
    let uri = format!("/api/projects/{PROJECT}/agent/sessions");
    let response = send(state, "POST", &uri, Some(body)).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    body_json(response).await["id"].as_str().unwrap().to_owned()
}

async fn sessions(state: &AppState) -> Vec<Value> {
    json_of(state, &format!("/api/projects/{PROJECT}/agent/sessions"))
        .await
        .as_array()
        .unwrap()
        .clone()
}

async fn next_command(started: &mut Started) -> Command {
    tokio::time::timeout(PATIENCE, started.commands.recv())
        .await
        .expect("the daemon sent no command in time")
        .expect("the command channel is open")
}

async fn feed(started: &Started, event: AgentEvent) {
    started
        .events
        .send(event)
        .await
        .expect("the pump is reading");
}

async fn session_stream(state: &AppState, id: &str) -> EventStream {
    let uri = format!("/api/projects/{PROJECT}/agent/sessions/{id}/events");
    EventStream::of(send(state, "GET", &uri, None).await)
}

async fn snapshot(state: &AppState, id: &str) -> Value {
    session_stream(state, id).await.expect().await.data
}

fn turn_started(prompt: &str) -> AgentEvent {
    AgentEvent::TurnStarted {
        turn: TurnId("1".to_owned()),
        prompt: prompt.to_owned(),
    }
}

fn turn_ended() -> AgentEvent {
    AgentEvent::TurnEnded {
        turn: TurnId("1".to_owned()),
        stop: TurnStop::Completed,
    }
}

// The CLI names the agent that ran it in a header, and the revision carries the name to the pump.
async fn task_written_via(state: &AppState, title: &str, agent: Option<&str>) -> String {
    let headers: Vec<(&str, &str)> = agent
        .map(|agent| (op_api::AGENT_HEADER, agent))
        .into_iter()
        .collect();
    let response = send_as(
        state,
        "POST",
        &format!("/api/projects/{PROJECT}/tasks"),
        Some(json!({ "title": title })),
        &headers,
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    body_json(response).await["id"].as_str().unwrap().to_owned()
}

#[tokio::test]
async fn a_post_starts_the_session_and_sends_the_prompt() {
    let (dir, state, agents) = with_agent();

    let id = start_session(&state, json!({ "prompt": "write me a task" })).await;
    let mut started = agents.recv().unwrap();

    assert_eq!(
        next_command(&mut started).await,
        Command::Prompt("write me a task".to_owned())
    );
    feed(&started, turn_started("write me a task")).await;
    until(|| async { sessions(&state).await[0]["status"]["kind"] == "running" }).await;

    let snapshot = snapshot(&state, &id).await;
    assert_eq!(snapshot["kind"], "snapshot");
    assert_eq!(snapshot["id"], id.as_str());
    assert_eq!(snapshot["project"], PROJECT);
    assert_eq!(
        snapshot["cwd"],
        dir.path().canonicalize().unwrap().display().to_string()
    );
    assert_eq!(
        snapshot["transcript"]["entries"][0]["text"],
        "write me a task"
    );
}

#[tokio::test]
async fn the_stream_opens_with_a_snapshot_and_then_carries_live_events() {
    let (_dir, state, agents) = with_agent();

    let id = start_session(&state, json!({ "prompt": "hello" })).await;
    let started = agents.recv().unwrap();

    let mut early = session_stream(&state, &id).await;
    assert_eq!(early.expect().await.data["kind"], "snapshot");

    feed(&started, turn_started("hello")).await;
    feed(
        &started,
        AgentEvent::MessageEnded {
            item: ItemId("m1".to_owned()),
            text: "here it is".to_owned(),
        },
    )
    .await;

    let first = early.expect().await.data;
    assert_eq!(first["kind"], "agent");
    assert_eq!(first["event"], "turn_started");
    assert_eq!(early.expect().await.data["event"], "message_ended");

    let late = snapshot(&state, &id).await;
    assert_eq!(late["kind"], "snapshot");
    let entries = late["transcript"]["entries"].as_array().unwrap();
    assert_eq!(entries[0]["kind"], "prompt");
    assert_eq!(entries[0]["text"], "hello");
    assert_eq!(entries[1]["kind"], "message");
    assert_eq!(entries[1]["text"], "here it is");
}

#[tokio::test]
async fn a_task_the_agent_writes_during_a_turn_binds_the_session() {
    let (_dir, state, agents) = with_agent();

    let id = start_session(&state, json!({ "prompt": "write me a task" })).await;
    let started = agents.recv().unwrap();
    let mut stream = session_stream(&state, &id).await;
    assert_eq!(stream.expect().await.data["kind"], "snapshot");

    feed(&started, turn_started("write me a task")).await;
    assert_eq!(stream.expect().await.data["event"], "turn_started");

    let key = task_written_via(&state, "A task the agent wrote", Some("claude-code")).await;

    let bound = stream.expect().await.data;
    assert_eq!(bound, json!({ "kind": "task", "id": key }));
    assert_eq!(sessions(&state).await[0]["task"], key.as_str());
}

// Only the agent that runs the session binds it, so a person's edit, or another agent's, in the
// same seconds binds nothing.
#[tokio::test]
async fn a_task_someone_else_writes_binds_nothing() {
    let (_dir, state, agents) = with_agent();

    let id = start_session(&state, json!({ "prompt": "write me a task" })).await;
    let started = agents.recv().unwrap();
    feed(&started, turn_started("write me a task")).await;
    until(|| async { sessions(&state).await[0]["status"]["kind"] == "running" }).await;

    let mut changes = EventStream::open(&state, None).await;
    task_written_via(&state, "A task a person wrote", None).await;
    task_written_via(&state, "A task another agent wrote", Some("codex")).await;
    changes.find("task_changed").await;
    changes.find("task_changed").await;
    feed(
        &started,
        AgentEvent::MessageEnded {
            item: ItemId("m1".to_owned()),
            text: "done".to_owned(),
        },
    )
    .await;

    // Both change events reached the session before the message did.
    until(|| async {
        snapshot(&state, &id).await["transcript"]["entries"]
            .as_array()
            .is_some_and(|entries| entries.len() == 2)
    })
    .await;
    assert!(snapshot(&state, &id).await["task"].is_null());
    assert!(sessions(&state).await[0]["task"].is_null());
}

#[tokio::test]
async fn a_task_the_agent_writes_between_turns_binds_nothing() {
    let (_dir, state, agents) = with_agent();

    let id = start_session(&state, json!({ "prompt": "go" })).await;
    let started = agents.recv().unwrap();
    feed(&started, turn_started("go")).await;
    feed(&started, turn_ended()).await;
    until(|| async { sessions(&state).await[0]["status"]["kind"] == "idle" }).await;

    let mut changes = EventStream::open(&state, None).await;
    task_written_via(&state, "Late", Some("claude-code")).await;
    changes.find("task_changed").await;
    feed(
        &started,
        AgentEvent::MessageEnded {
            item: ItemId("m1".to_owned()),
            text: "late".to_owned(),
        },
    )
    .await;

    until(|| async {
        snapshot(&state, &id).await["transcript"]["entries"]
            .as_array()
            .is_some_and(|entries| entries.len() == 2)
    })
    .await;
    assert!(sessions(&state).await[0]["task"].is_null());
}

#[tokio::test]
async fn a_session_started_on_a_task_is_bound_before_its_first_event() {
    let (_dir, state, agents) = with_agent();
    let key = create(&state, "An existing task").await;

    let id = start_session(&state, json!({ "task": key, "prompt": "change it" })).await;
    let _started = agents.recv().unwrap();

    assert_eq!(sessions(&state).await[0]["task"], key.as_str());
    assert_eq!(snapshot(&state, &id).await["task"], key.as_str());
}

#[tokio::test]
async fn a_session_on_a_task_the_project_does_not_hold_is_refused() {
    let (_dir, state, _agents) = with_agent();
    let uri = format!("/api/projects/{PROJECT}/agent/sessions");

    let response = send(
        &state,
        "POST",
        &uri,
        Some(json!({ "task": "OPP-404", "prompt": "change it" })),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let response = send(
        &state,
        "POST",
        &uri,
        Some(json!({ "task": "nonsense", "prompt": "change it" })),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(sessions(&state).await.is_empty());
}

#[tokio::test]
async fn an_agent_the_daemon_does_not_run_is_unavailable() {
    let (_dir, state, _agents) = with_agent();
    let response = send(
        &state,
        "POST",
        &format!("/api/projects/{PROJECT}/agent/sessions"),
        Some(json!({ "agent": "codex", "prompt": "go" })),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(message_of(&body_json(response).await).contains("codex"));
}

#[tokio::test]
async fn a_prompt_waits_for_the_running_turn() {
    let (_dir, state, agents) = with_agent();

    let id = start_session(&state, json!({ "prompt": "first" })).await;
    let mut started = agents.recv().unwrap();
    assert_eq!(
        next_command(&mut started).await,
        Command::Prompt("first".to_owned())
    );

    feed(&started, turn_started("first")).await;
    let uri = format!("/api/projects/{PROJECT}/agent/sessions/{id}/prompt");
    until(|| async {
        send(&state, "POST", &uri, Some(json!({ "text": "second" })))
            .await
            .status()
            == StatusCode::CONFLICT
    })
    .await;

    feed(&started, turn_ended()).await;
    until(|| async {
        send(&state, "POST", &uri, Some(json!({ "text": "second" })))
            .await
            .status()
            == StatusCode::ACCEPTED
    })
    .await;
    assert_eq!(
        next_command(&mut started).await,
        Command::Prompt("second".to_owned())
    );
}

#[tokio::test]
async fn an_interrupt_reaches_the_agent() {
    let (_dir, state, agents) = with_agent();

    let id = start_session(&state, json!({ "prompt": "go" })).await;
    let mut started = agents.recv().unwrap();
    assert_eq!(
        next_command(&mut started).await,
        Command::Prompt("go".to_owned())
    );

    let uri = format!("/api/projects/{PROJECT}/agent/sessions/{id}/interrupt");
    assert_eq!(
        send(&state, "POST", &uri, None).await.status(),
        StatusCode::ACCEPTED
    );
    assert_eq!(next_command(&mut started).await, Command::Interrupt);
}

#[tokio::test]
async fn an_approval_decision_reaches_the_agent_and_an_unknown_one_does_not() {
    let (_dir, state, agents) = with_agent();

    let id = start_session(&state, json!({ "prompt": "go" })).await;
    let mut started = agents.recv().unwrap();
    assert_eq!(
        next_command(&mut started).await,
        Command::Prompt("go".to_owned())
    );

    feed(&started, turn_started("go")).await;
    feed(
        &started,
        AgentEvent::ApprovalRequested(ApprovalRequest {
            id: ApprovalId("a1".to_owned()),
            item: Some(ItemId("t1".to_owned())),
            tool: "Bash".to_owned(),
            input: json!({ "command": "openplan create \"A\"" }),
            reason: None,
        }),
    )
    .await;

    let uri = format!("/api/projects/{PROJECT}/agent/sessions/{id}/approvals/a1");
    until(|| async {
        send(&state, "POST", &uri, Some(json!("allow")))
            .await
            .status()
            == StatusCode::ACCEPTED
    })
    .await;
    assert_eq!(
        next_command(&mut started).await,
        Command::Approve {
            id: ApprovalId("a1".to_owned()),
            decision: op_agent::ApprovalDecision::Allow,
        }
    );

    let unknown = format!("/api/projects/{PROJECT}/agent/sessions/{id}/approvals/a2");
    let response = send(&state, "POST", &unknown, Some(json!("allow"))).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_delete_stops_the_agent_and_takes_the_entry_when_it_exits() {
    let (_dir, state, agents) = with_agent();

    let id = start_session(&state, json!({ "prompt": "go" })).await;
    let mut started = agents.recv().unwrap();
    assert_eq!(
        next_command(&mut started).await,
        Command::Prompt("go".to_owned())
    );

    let uri = format!("/api/projects/{PROJECT}/agent/sessions/{id}");
    assert_eq!(
        send(&state, "DELETE", &uri, None).await.status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(next_command(&mut started).await, Command::Shutdown);
    assert_eq!(sessions(&state).await.len(), 1);

    feed(&started, AgentEvent::Exited { code: Some(0) }).await;
    drop(started);
    until(|| async { sessions(&state).await.is_empty() }).await;
}

#[tokio::test]
async fn an_agent_that_exits_on_its_own_keeps_its_entry() {
    let (_dir, state, agents) = with_agent();

    let id = start_session(&state, json!({ "prompt": "go" })).await;
    let started = agents.recv().unwrap();

    feed(&started, AgentEvent::Exited { code: Some(2) }).await;
    drop(started);

    until(|| async { sessions(&state).await[0]["status"]["kind"] == "exited" }).await;
    let listed = sessions(&state).await;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0]["id"], id.as_str());
    assert_eq!(listed[0]["status"]["code"], 2);
}

#[tokio::test]
async fn the_daemons_stop_asks_every_session_to_stop() {
    let (_dir, state, agents) = with_agent();

    start_session(&state, json!({ "prompt": "one" })).await;
    let mut first = agents.recv().unwrap();
    start_session(&state, json!({ "prompt": "two" })).await;
    let mut second = agents.recv().unwrap();
    assert_eq!(
        next_command(&mut first).await,
        Command::Prompt("one".to_owned())
    );
    assert_eq!(
        next_command(&mut second).await,
        Command::Prompt("two".to_owned())
    );

    state.stop();

    assert_eq!(next_command(&mut first).await, Command::Shutdown);
    assert_eq!(next_command(&mut second).await, Command::Shutdown);
}

#[tokio::test]
async fn an_update_waits_until_no_session_runs() {
    let (_dir, state, agents) = with_agent();
    start_session(&state, json!({ "prompt": "go" })).await;
    let started = agents.recv().unwrap();
    let mut installs = 0;

    let updated = state.update_if_idle(|| {
        installs += 1;
        Ok::<(), ()>(())
    });

    assert_eq!(updated, Ok(false));
    assert_eq!(installs, 0);
    assert_eq!(state.stop_reason(), None);

    feed(&started, AgentEvent::Exited { code: Some(0) }).await;
    drop(started);
    let deadline = tokio::time::Instant::now() + PATIENCE;
    while state.update_if_idle(|| {
        installs += 1;
        Ok::<(), ()>(())
    }) != Ok(true)
    {
        assert!(
            tokio::time::Instant::now() < deadline,
            "the session did not end in time"
        );
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert_eq!(installs, 1);
    assert_eq!(state.stop_reason(), Some(op_api::StopReason::Update));
}

#[tokio::test]
async fn a_failed_install_leaves_the_daemon_running() {
    let (_dir, state, _agents) = with_agent();

    assert_eq!(
        state.update_if_idle(|| Err("no space left")),
        Err("no space left")
    );
    assert_eq!(state.stop_reason(), None);
}

#[tokio::test]
async fn no_session_starts_once_an_update_stops_the_daemon() {
    let (_dir, state, agents) = with_agent();

    assert_eq!(state.update_if_idle(|| Ok::<(), ()>(())), Ok(true));
    let uri = format!("/api/projects/{PROJECT}/agent/sessions");
    let response = send(&state, "POST", &uri, Some(json!({ "prompt": "go" }))).await;

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let mut started = agents.recv().unwrap();
    assert_eq!(next_command(&mut started).await, Command::Shutdown);
    assert!(sessions(&state).await.is_empty());
}

#[tokio::test]
async fn the_list_answers_newest_first() {
    let (_dir, state, agents) = with_agent();

    let first = start_session(&state, json!({ "prompt": "one" })).await;
    let _first = agents.recv().unwrap();
    let second = start_session(&state, json!({ "prompt": "two" })).await;
    let _second = agents.recv().unwrap();

    let listed = sessions(&state).await;
    assert_eq!(listed[0]["id"], second.as_str());
    assert_eq!(listed[1]["id"], first.as_str());
}

#[tokio::test]
async fn an_unknown_session_is_not_found() {
    let (_dir, state, _agents) = with_agent();

    for (method, path) in [("GET", "events"), ("POST", "interrupt"), ("DELETE", "")] {
        let uri = format!("/api/projects/{PROJECT}/agent/sessions/deadbeef/{path}");
        assert_eq!(
            send(&state, method, &uri, None).await.status(),
            StatusCode::NOT_FOUND,
            "{method} {path}"
        );
    }
}

#[tokio::test]
async fn a_session_of_an_unknown_project_is_not_found() {
    let (_dir, state, _agents) = with_agent();
    let response = send(
        &state,
        "POST",
        "/api/projects/ghost/agent/sessions",
        Some(json!({ "prompt": "go" })),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// The tasks are not files in the checkout, so the agent works in the checkout only to read the code
// and writes every task through the CLI.
#[tokio::test]
async fn the_instructions_name_the_checkout_the_binary_and_the_task() {
    let (dir, state, agents) = with_agent();
    let key = create(&state, "An existing task").await;

    start_session(&state, json!({ "task": key, "prompt": "change it" })).await;
    let started = agents.recv().unwrap();
    let instructions = started
        .options
        .instructions
        .expect("the daemon appends them");

    let checkout = dir.path().canonicalize().unwrap();
    assert_eq!(started.options.cwd, checkout);
    assert!(instructions.contains(&checkout.display().to_string()));
    assert!(instructions.contains(&std::env::current_exe().unwrap().display().to_string()));
    assert!(instructions.contains(&key));
    assert!(instructions.contains("openplan CLI"));
    assert!(instructions.contains("Never run git"));
}
