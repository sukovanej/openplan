mod common;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use axum::http::StatusCode;
use common::*;
use op_agent::session::Channel;
use op_agent::{
    Agent, AgentError, AgentEvent, AgentKind, ApprovalId, ApprovalRequest, Command, ItemId,
    Persistence, Session, SessionId, SessionInfo, SessionOptions, ToolCall, ToolOutcome, TurnId,
    TurnStop,
};
use op_api::BackendKind;
use op_server::{AppState, Project};
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

// A daemon whose agent sessions live in `store`. A second one over the same file is the daemon after
// a restart.
fn restartable(project: Project, store: &Path) -> (AppState, std::sync::mpsc::Receiver<Started>) {
    let (tx, rx) = std::sync::mpsc::channel();
    let fake = Arc::new(Fake {
        kind: AgentKind::ClaudeCode,
        started: Mutex::new(tx),
    });
    let state = AppState::new([project])
        .with_agents(BTreeMap::from([(
            AgentKind::ClaudeCode,
            fake as Arc<dyn Agent>,
        )]))
        .with_agent_store(store)
        .expect("the store opens");
    state.start_projects();
    (state, rx)
}

fn ready(session: &str) -> AgentEvent {
    AgentEvent::Ready(SessionInfo {
        session: SessionId(session.to_owned()),
        cwd: "/w".into(),
        model: None,
        tools: Vec::new(),
    })
}

async fn start_session(state: &AppState, body: Value) -> String {
    let uri = format!("/api/projects/{PROJECT}/agent/sessions");
    let response = send(state, "POST", &uri, Some(body)).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    body_json(response).await["id"].as_str().unwrap().to_owned()
}

async fn sessions(state: &AppState) -> Vec<Value> {
    json_of(state, "/api/agent/sessions")
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

async fn tool(started: &Started, id: &str, name: &str, input: Value, output: &str, failed: bool) {
    feed(
        started,
        AgentEvent::ToolStarted(ToolCall {
            item: ItemId(id.to_owned()),
            name: name.to_owned(),
            input,
        }),
    )
    .await;
    feed(
        started,
        AgentEvent::ToolEnded(ToolOutcome {
            item: ItemId(id.to_owned()),
            output: output.to_owned(),
            failed,
        }),
    )
    .await;
}

// A message after the tools is what tells a test the pump has applied everything ahead of it.
async fn settle(state: &AppState, started: &Started, id: &str) {
    feed(
        started,
        AgentEvent::MessageEnded {
            item: ItemId("settled".to_owned()),
            text: "done".to_owned(),
        },
    )
    .await;
    until(|| async {
        snapshot(state, id).await["transcript"]["entries"]
            .as_array()
            .is_some_and(|entries| entries.iter().any(|entry| entry["item"] == "settled"))
    })
    .await;
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
async fn a_cli_write_marks_the_task_it_names() {
    let (_dir, state, agents) = with_agent();

    let id = start_session(&state, json!({ "prompt": "write me a task" })).await;
    let started = agents.recv().unwrap();
    let mut stream = session_stream(&state, &id).await;
    assert_eq!(stream.expect().await.data["kind"], "snapshot");

    feed(&started, turn_started("write me a task")).await;
    tool(
        &started,
        "t1",
        "Bash",
        json!({ "command": "/usr/bin/openplan create \"Close on Escape\" --parent OPP-1" }),
        "OPP-9\n",
        false,
    )
    .await;
    tool(
        &started,
        "t2",
        "shell",
        json!({ "command": ["openplan", "set", "OPP-2", "status", "done"] }),
        "",
        false,
    )
    .await;
    tool(
        &started,
        "t3",
        "Bash",
        json!({ "command": "openplan write OPP-5 --file /tmp/task.md" }),
        "",
        false,
    )
    .await;

    let tasks = loop {
        let event = stream.expect().await.data;
        if event["kind"] == "tasks"
            && event["tasks"]
                .as_array()
                .is_some_and(|tasks| tasks.len() == 3)
        {
            break event;
        }
    };
    assert_eq!(tasks["tasks"], json!(["OPP-9", "OPP-2", "OPP-5"]));
    assert_eq!(
        sessions(&state).await[0]["tasks"],
        json!(["OPP-9", "OPP-2", "OPP-5"])
    );
}

#[tokio::test]
async fn a_read_a_search_or_a_failed_write_marks_nothing() {
    let (_dir, state, agents) = with_agent();

    let id = start_session(&state, json!({ "prompt": "what blocks OPP-3" })).await;
    let started = agents.recv().unwrap();
    feed(&started, turn_started("what blocks OPP-3")).await;
    tool(
        &started,
        "t1",
        "Bash",
        json!({ "command": "openplan get OPP-3" }),
        "",
        false,
    )
    .await;
    tool(
        &started,
        "t2",
        "Bash",
        json!({ "command": "openplan search escape" }),
        "OPP-3\nOPP-4\n",
        false,
    )
    .await;
    tool(
        &started,
        "t3",
        "Bash",
        json!({ "command": "openplan set OPP-5 status done" }),
        "denied",
        true,
    )
    .await;
    settle(&state, &started, &id).await;

    assert_eq!(sessions(&state).await[0]["tasks"], json!([]));
}

// The CLI names only the kind of agent that wrote, so the session reads its own tool calls.
#[tokio::test]
async fn two_sessions_that_run_at_once_keep_their_own_tasks() {
    let (_dir, state, agents) = with_agent();

    let first = start_session(&state, json!({ "prompt": "one" })).await;
    let one = agents.recv().unwrap();
    let second = start_session(&state, json!({ "prompt": "two" })).await;
    let two = agents.recv().unwrap();
    feed(&one, turn_started("one")).await;
    feed(&two, turn_started("two")).await;

    let set = |key: &str| json!({ "command": format!("openplan set {key} status done") });
    tool(&one, "t1", "Bash", set("OPP-3"), "", false).await;
    tool(&two, "t1", "Bash", set("OPP-4"), "", false).await;
    settle(&state, &one, &first).await;
    settle(&state, &two, &second).await;

    let listed = sessions(&state).await;
    let of = |id: &str| listed.iter().find(|session| session["id"] == id).unwrap()["tasks"].clone();
    assert_eq!(of(&first), json!(["OPP-3"]));
    assert_eq!(of(&second), json!(["OPP-4"]));
}

#[tokio::test]
async fn a_session_carries_its_title_and_the_task_open_when_it_started() {
    let (_dir, state, agents) = with_agent();
    let key = create(&state, "An existing task").await;

    let id = start_session(
        &state,
        json!({ "context": key, "prompt": "why is it blocked" }),
    )
    .await;
    let _started = agents.recv().unwrap();

    let summary = &sessions(&state).await[0];
    assert_eq!(summary["context"], key.as_str());
    assert_eq!(summary["title"], "why is it blocked");
    assert_eq!(summary["project"], PROJECT);
    assert_eq!(summary["tasks"], json!([]));
    let snapshot = snapshot(&state, &id).await;
    assert_eq!(snapshot["context"], key.as_str());
    assert_eq!(snapshot["title"], "why is it blocked");
}

#[tokio::test]
async fn the_change_stream_announces_a_new_session_and_each_new_status() {
    let (_dir, state, agents) = with_agent();
    let mut changes = EventStream::open(&state, None).await;

    start_session(&state, json!({ "prompt": "go" })).await;
    let started = agents.recv().unwrap();
    assert_eq!(
        changes.find("agent_sessions_changed").await.data["project"],
        PROJECT
    );

    feed(&started, turn_started("go")).await;
    assert_eq!(
        changes.find("agent_sessions_changed").await.data["project"],
        PROJECT
    );
}

#[tokio::test]
async fn a_session_on_a_task_the_project_does_not_hold_is_refused() {
    let (_dir, state, _agents) = with_agent();
    let uri = format!("/api/projects/{PROJECT}/agent/sessions");

    let response = send(
        &state,
        "POST",
        &uri,
        Some(json!({ "context": "OPP-404", "prompt": "change it" })),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let response = send(
        &state,
        "POST",
        &uri,
        Some(json!({ "context": "nonsense", "prompt": "change it" })),
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
async fn a_delete_marks_the_session_done_and_stops_its_agent() {
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
    assert!(sessions(&state).await.is_empty());
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
async fn a_delete_takes_the_entry_of_an_agent_that_already_exited() {
    let (_dir, state, agents) = with_agent();

    let id = start_session(&state, json!({ "prompt": "go" })).await;
    let started = agents.recv().unwrap();
    feed(&started, AgentEvent::Exited { code: Some(1) }).await;
    drop(started);
    until(|| async { sessions(&state).await[0]["status"]["kind"] == "exited" }).await;

    let uri = format!("/api/projects/{PROJECT}/agent/sessions/{id}");
    assert_eq!(
        send(&state, "DELETE", &uri, None).await.status(),
        StatusCode::NO_CONTENT
    );
    until(|| async { sessions(&state).await.is_empty() }).await;
}

#[tokio::test]
async fn an_agent_that_exited_resumes_its_session_on_the_next_prompt() {
    let (_dir, state, agents) = with_agent();

    let id = start_session(&state, json!({ "prompt": "one" })).await;
    let started = agents.recv().unwrap();
    feed(&started, ready("claude-9")).await;
    feed(&started, AgentEvent::Exited { code: Some(1) }).await;
    until(|| async { sessions(&state).await[0]["status"]["kind"] == "exited" }).await;

    let uri = format!("/api/projects/{PROJECT}/agent/sessions/{id}/prompt");
    let response = send(&state, "POST", &uri, Some(json!({ "text": "two" }))).await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let mut resumed = agents.recv().unwrap();
    assert_eq!(
        resumed.options.resume,
        Some(SessionId("claude-9".to_owned()))
    );
    assert_eq!(
        next_command(&mut resumed).await,
        Command::Prompt("two".to_owned())
    );

    // The first agent's pump drains after the second one started, and leaves its handle alone.
    drop(started);
    feed(&resumed, turn_started("two")).await;
    until(|| async { sessions(&state).await[0]["status"]["kind"] == "running" }).await;
    let uri = format!("/api/projects/{PROJECT}/agent/sessions/{id}/interrupt");
    assert_eq!(
        send(&state, "POST", &uri, None).await.status(),
        StatusCode::ACCEPTED
    );
    assert_eq!(next_command(&mut resumed).await, Command::Interrupt);
}

#[tokio::test]
async fn a_restarted_daemon_brings_back_its_sessions_stopped_and_whole() {
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("agent-sessions.sqlite3");
    let (first, agents) = restartable(local_project(PROJECT, dir.path(), "OPP"), &store);
    let key = create(&first, "An existing task").await;

    let id = start_session(&first, json!({ "context": key, "prompt": "retitle it" })).await;
    let started = agents.recv().unwrap();
    feed(&started, ready("claude-1")).await;
    feed(&started, turn_started("retitle it")).await;
    tool(
        &started,
        "t1",
        "Bash",
        json!({ "command": "openplan set OPP-7 title Retitled" }),
        "",
        false,
    )
    .await;
    feed(
        &started,
        AgentEvent::MessageEnded {
            item: ItemId("m1".to_owned()),
            text: "Retitled.".to_owned(),
        },
    )
    .await;
    feed(&started, turn_ended()).await;
    until(|| async { sessions(&first).await[0]["status"]["kind"] == "idle" }).await;
    drop(started);
    first.agents().finish().await;

    let (second, _agents) = restartable(open(PROJECT, dir.path(), BackendKind::Local), &store);
    let listed = sessions(&second).await;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0]["id"], id.as_str());
    assert_eq!(listed[0]["title"], "retitle it");
    assert_eq!(listed[0]["context"], key.as_str());
    assert_eq!(listed[0]["tasks"], json!(["OPP-7"]));
    assert_eq!(
        listed[0]["status"],
        json!({ "kind": "exited", "code": null })
    );
    let entries = snapshot(&second, &id).await["transcript"]["entries"].clone();
    let kinds: Vec<&str> = entries
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["prompt", "tool", "message"]);
    assert_eq!(entries[2]["text"], "Retitled.");
}

#[tokio::test]
async fn a_prompt_to_a_restored_session_resumes_the_agent_session_it_had() {
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("agent-sessions.sqlite3");
    let (first, agents) = restartable(local_project(PROJECT, dir.path(), "OPP"), &store);

    let id = start_session(&first, json!({ "prompt": "one" })).await;
    let started = agents.recv().unwrap();
    feed(&started, ready("claude-1")).await;
    feed(&started, turn_started("one")).await;
    feed(&started, turn_ended()).await;
    until(|| async { sessions(&first).await[0]["status"]["kind"] == "idle" }).await;
    drop(started);
    first.agents().finish().await;

    let (second, agents) = restartable(open(PROJECT, dir.path(), BackendKind::Local), &store);
    let uri = format!("/api/projects/{PROJECT}/agent/sessions/{id}/prompt");
    let response = send(&second, "POST", &uri, Some(json!({ "text": "two" }))).await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let mut resumed = agents.recv().unwrap();
    assert_eq!(
        resumed.options.resume,
        Some(SessionId("claude-1".to_owned()))
    );
    assert_eq!(resumed.options.persistence, Persistence::Stored);
    assert_eq!(
        next_command(&mut resumed).await,
        Command::Prompt("two".to_owned())
    );
    assert_eq!(sessions(&second).await[0]["status"]["kind"], "starting");
}

#[tokio::test]
async fn a_session_marked_done_stays_gone_after_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("agent-sessions.sqlite3");
    let (first, agents) = restartable(local_project(PROJECT, dir.path(), "OPP"), &store);

    let kept = start_session(&first, json!({ "prompt": "keep me" })).await;
    let kept_agent = agents.recv().unwrap();
    let done = start_session(&first, json!({ "prompt": "done with me" })).await;
    let done_agent = agents.recv().unwrap();
    let uri = format!("/api/projects/{PROJECT}/agent/sessions/{done}");
    assert_eq!(
        send(&first, "DELETE", &uri, None).await.status(),
        StatusCode::NO_CONTENT
    );
    drop(kept_agent);
    drop(done_agent);
    first.agents().finish().await;

    let (second, _agents) = restartable(open(PROJECT, dir.path(), BackendKind::Local), &store);
    let listed = sessions(&second).await;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0]["id"], kept.as_str());
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
async fn the_instructions_name_the_checkout_the_binary_and_the_open_task() {
    let (dir, state, agents) = with_agent();
    let key = create(&state, "An existing task").await;

    start_session(&state, json!({ "context": key, "prompt": "change it" })).await;
    let started = agents.recv().unwrap();
    let instructions = started
        .options
        .instructions
        .expect("the daemon appends them");

    let checkout = dir.path().canonicalize().unwrap();
    assert_eq!(started.options.cwd, checkout);
    assert_eq!(started.options.model.as_deref(), Some("sonnet"));
    assert_eq!(started.options.persistence, Persistence::Stored);
    assert!(instructions.contains(&checkout.display().to_string()));
    let exe = std::env::current_exe().unwrap().display().to_string();
    assert!(instructions.contains(&exe));
    assert!(instructions.contains("openplan CLI"));
    assert!(instructions.contains(&format!("The user has task {key} open.")));
    assert!(instructions.contains(&format!("`{exe} get {key}`")));
    assert!(instructions.contains(&format!("`{exe} write {key} --file <file>`")));
    assert!(instructions.contains(&format!("`{exe} set {key} <field> <value>`")));
    assert!(!instructions.contains(".plan/tasks"));
    assert!(instructions.contains("Never run git"));
}

#[tokio::test]
async fn the_instructions_name_the_registered_tags_and_forbid_code() {
    let (_dir, state, agents) = with_agent();

    start_session(&state, json!({ "prompt": "the box stays open" })).await;
    let instructions = agents
        .recv()
        .unwrap()
        .options
        .instructions
        .expect("the daemon appends them");

    assert!(instructions.contains(&format!("registered tags: {}.", DEFAULT_TAGS.join(", "))));
    assert!(instructions.contains("You never write code"));
}
