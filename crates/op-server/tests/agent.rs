use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::response::Response;
use http_body_util::BodyExt;
use op_agent::session::Channel;
use op_agent::{
    Agent, AgentError, AgentEvent, AgentKind, ApprovalId, ApprovalRequest, Command, ItemId,
    Session, SessionOptions, TurnId, TurnStop,
};
use op_server::{AppState, Project, app};
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tower::ServiceExt;

const PROJECT: &str = "test";
const PATIENCE: Duration = Duration::from_secs(5);

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

fn git(dir: &std::path::Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .current_dir(dir)
        .args(args)
        .status()
        .expect("git must be installed for this test");
    assert!(status.success(), "git {args:?} failed");
}

fn repository(dir: &std::path::Path) {
    git(dir, &["init", "-q", "-b", "main"]);
    git(dir, &["config", "user.email", "t@example.com"]);
    git(dir, &["config", "user.name", "Test"]);
    std::fs::create_dir_all(dir.join(".plan/tasks")).unwrap();
    std::fs::write(dir.join(".plan/config.toml"), "abbreviation = \"OPP\"\n").unwrap();
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-qm", "init"]);
}

fn state_of(root: &std::path::Path) -> AppState {
    let store = op_store::Store::discover(root).unwrap();
    let repo = op_git::Repo::discover(root).unwrap();
    let config = op_store::Config {
        abbreviation: store.abbreviation(),
        default_branch: None,
    };
    AppState::new([Project::new(
        PROJECT,
        root.to_path_buf(),
        repo,
        store,
        &config,
    )])
}

fn with_agent() -> (
    tempfile::TempDir,
    AppState,
    std::sync::mpsc::Receiver<Started>,
) {
    let dir = tempfile::tempdir().unwrap();
    repository(dir.path());
    let (tx, rx) = std::sync::mpsc::channel();
    let fake = Arc::new(Fake {
        kind: AgentKind::ClaudeCode,
        started: Mutex::new(tx),
    });
    let state = state_of(dir.path()).with_agents(BTreeMap::from([(
        AgentKind::ClaudeCode,
        fake as Arc<dyn Agent>,
    )]));
    state.start_watchers();
    (dir, state, rx)
}

async fn send(state: &AppState, method: &str, uri: &str, body: Option<Value>) -> Response {
    let builder = Request::builder().method(method).uri(uri);
    let request = match body {
        Some(value) => builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_vec(&value).unwrap()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    };
    app(state.clone()).oneshot(request).await.unwrap()
}

async fn body_json(response: Response) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

async fn start_session(state: &AppState, body: Value) -> String {
    let uri = format!("/api/projects/{PROJECT}/agent/sessions");
    let response = send(state, "POST", &uri, Some(body)).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    body_json(response).await["id"].as_str().unwrap().to_owned()
}

async fn sessions(state: &AppState) -> Vec<Value> {
    let uri = format!("/api/projects/{PROJECT}/agent/sessions");
    let response = send(state, "GET", &uri, None).await;
    assert_eq!(response.status(), StatusCode::OK);
    body_json(response).await.as_array().unwrap().clone()
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

// The pump applies an event on its own task, so a read that follows a send has to wait for it.
async fn until<F, Fut>(mut ready: F)
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let deadline = std::time::Instant::now() + PATIENCE;
    loop {
        if ready().await {
            return;
        }
        assert!(std::time::Instant::now() < deadline, "the state never came");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

// One open event stream, read one event at a time, so a test can watch the live events and then
// open a second stream and compare what a late reader is given.
struct Stream {
    body: Body,
    buffer: String,
}

impl Stream {
    async fn open(state: &AppState, id: &str) -> Self {
        let uri = format!("/api/projects/{PROJECT}/agent/sessions/{id}/events");
        let response = send(state, "GET", &uri, None).await;
        assert_eq!(response.status(), StatusCode::OK);
        Self {
            body: response.into_body(),
            buffer: String::new(),
        }
    }

    async fn next(&mut self) -> Value {
        loop {
            if let Some(end) = self.buffer.find("\n\n") {
                let event: String = self.buffer.drain(..end + 2).collect();
                if let Some(line) = event.lines().find_map(|line| line.strip_prefix("data:")) {
                    return serde_json::from_str(line.trim()).expect("an SSE event carries JSON");
                }
                continue;
            }
            let frame = tokio::time::timeout(PATIENCE, self.body.frame())
                .await
                .expect("the stream sent no frame in time")
                .expect("the stream is open")
                .unwrap();
            if let Some(data) = frame.data_ref() {
                self.buffer.push_str(&String::from_utf8_lossy(data));
            }
        }
    }
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

async fn task_on_branch(state: &AppState, title: &str, branch: &str) -> String {
    let uri = format!("/api/projects/{PROJECT}/tasks?branch={branch}");
    let response = send(state, "POST", &uri, Some(json!({ "title": title }))).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    body_json(response).await["id"].as_str().unwrap().to_owned()
}

#[tokio::test]
async fn a_post_starts_the_session_and_sends_the_prompt() {
    let (_dir, state, agents) = with_agent();

    let id = start_session(&state, json!({ "prompt": "write me a task" })).await;
    let mut started = agents.recv().unwrap();

    assert_eq!(
        next_command(&mut started).await,
        Command::Prompt("write me a task".to_owned())
    );
    feed(&started, turn_started("write me a task")).await;
    until(|| async { sessions(&state).await[0]["status"]["kind"] == "running" }).await;

    let snapshot = Stream::open(&state, &id).await.next().await;
    assert_eq!(snapshot["kind"], "snapshot");
    assert_eq!(snapshot["id"], id.as_str());
    assert_eq!(snapshot["branch"], op_git::ROLLING_UPDATES_BRANCH);
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

    let mut early = Stream::open(&state, &id).await;
    assert_eq!(early.next().await["kind"], "snapshot");

    feed(&started, turn_started("hello")).await;
    feed(
        &started,
        AgentEvent::MessageEnded {
            item: ItemId("m1".to_owned()),
            text: "here it is".to_owned(),
        },
    )
    .await;

    let first = early.next().await;
    assert_eq!(first["kind"], "agent");
    assert_eq!(first["event"], "turn_started");
    let second = early.next().await;
    assert_eq!(second["event"], "message_ended");

    // The late reader saw neither event, and its snapshot has to draw the same page.
    let late = Stream::open(&state, &id).await.next().await;
    assert_eq!(late["kind"], "snapshot");
    let entries = late["transcript"]["entries"].as_array().unwrap();
    assert_eq!(entries[0]["kind"], "prompt");
    assert_eq!(entries[0]["text"], "hello");
    assert_eq!(entries[1]["kind"], "message");
    assert_eq!(entries[1]["text"], "here it is");
}

#[tokio::test]
async fn a_task_written_on_the_workspace_branch_binds_the_session() {
    let (_dir, state, agents) = with_agent();

    let id = start_session(&state, json!({ "prompt": "write me a task" })).await;
    let started = agents.recv().unwrap();
    let mut stream = Stream::open(&state, &id).await;
    assert_eq!(stream.next().await["kind"], "snapshot");

    feed(&started, turn_started("write me a task")).await;
    assert_eq!(stream.next().await["event"], "turn_started");

    let key = task_on_branch(
        &state,
        "A task the agent wrote",
        op_git::ROLLING_UPDATES_BRANCH,
    )
    .await;

    let bound = stream.next().await;
    assert_eq!(bound["kind"], "task");
    assert_eq!(bound["id"], key.as_str());
    assert_eq!(sessions(&state).await[0]["task"], key.as_str());
}

#[tokio::test]
async fn a_task_written_on_another_branch_binds_nothing() {
    let (_dir, state, agents) = with_agent();

    let id = start_session(&state, json!({ "prompt": "write me a task" })).await;
    let started = agents.recv().unwrap();
    feed(&started, turn_started("write me a task")).await;
    until(|| async { sessions(&state).await[0]["status"]["kind"] == "running" }).await;

    task_on_branch(&state, "A task on the default branch", "main").await;
    feed(
        &started,
        AgentEvent::MessageEnded {
            item: ItemId("m1".to_owned()),
            text: "done".to_owned(),
        },
    )
    .await;

    // The message is what tells us the change event has been through the pump ahead of it.
    until(|| async {
        Stream::open(&state, &id).await.next().await["transcript"]["entries"]
            .as_array()
            .is_some_and(|entries| entries.len() == 2)
    })
    .await;
    assert!(Stream::open(&state, &id).await.next().await["task"].is_null());
    assert!(sessions(&state).await[0]["task"].is_null());
}

#[tokio::test]
async fn a_session_started_on_a_task_is_bound_before_its_first_event() {
    let (_dir, state, agents) = with_agent();
    let key = task_on_branch(&state, "An existing task", op_git::ROLLING_UPDATES_BRANCH).await;

    let id = start_session(&state, json!({ "task": key, "prompt": "change it" })).await;
    let _started = agents.recv().unwrap();

    assert_eq!(sessions(&state).await[0]["task"], key.as_str());
    assert_eq!(
        Stream::open(&state, &id).await.next().await["task"],
        key.as_str()
    );
}

#[tokio::test]
async fn a_session_on_a_task_the_branch_does_not_hold_is_refused() {
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
            tool: "Write".to_owned(),
            input: json!({ "path": ".plan/tasks/00001-a.md" }),
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
async fn a_repository_without_a_rolling_updates_branch_runs_no_agent() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "t@example.com"]);
    git(root, &["config", "user.name", "Test"]);
    std::fs::create_dir_all(root.join(".plan/tasks")).unwrap();
    std::fs::write(root.join(".plan/config.toml"), "abbreviation = \"OPP\"\n").unwrap();
    git(root, &["commit", "-q", "--allow-empty", "-m", "init"]);
    let state = state_of(root);
    state.start_watchers();

    let uri = format!("/api/projects/{PROJECT}/agent/sessions");
    assert_eq!(
        send(&state, "GET", &uri, None).await.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    let response = send(&state, "POST", &uri, Some(json!({ "prompt": "go" }))).await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let events = format!("/api/projects/{PROJECT}/agent/sessions/abc/events");
    assert_eq!(
        send(&state, "GET", &events, None).await.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
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
async fn the_instructions_name_the_worktree_the_code_root_the_binary_and_the_task() {
    let (dir, state, agents) = with_agent();
    let key = task_on_branch(&state, "An existing task", op_git::ROLLING_UPDATES_BRANCH).await;

    start_session(&state, json!({ "task": key, "prompt": "change it" })).await;
    let started = agents.recv().unwrap();
    let instructions = started
        .options
        .instructions
        .expect("the daemon appends them");

    let worktree = dir.path().join(".git/openplan-rolling-updates");
    assert_eq!(started.options.cwd, worktree);
    assert!(instructions.contains(&worktree.join(".plan/tasks").display().to_string()));
    assert!(instructions.contains(&dir.path().display().to_string()));
    assert!(instructions.contains(&std::env::current_exe().unwrap().display().to_string()));
    assert!(instructions.contains(&key));
    assert!(instructions.contains("Never run git"));
}

// The real binary, a real repository, and a real turn. Ignored because it costs tokens and needs
// `claude` on PATH: run it with `cargo test -p op-server --test agent -- --ignored`.
//
// The prompt asks for the file rather than for `<binary> create`, because the instructions name
// the running executable as the CLI and here that executable is this test.
#[tokio::test]
#[ignore]
async fn the_real_agent_writes_a_task_in_the_worktree() {
    let dir = tempfile::tempdir().unwrap();
    repository(dir.path());
    let state = state_of(dir.path());
    state.start_watchers();

    let id = start_session(
        &state,
        json!({
            "prompt": "Write one new task file in .plan/tasks. Name the file \
                       00001-sweep-the-yard.md. Give it the frontmatter keys `status: backlog` \
                       and `created: 2026-01-01T00:00:00Z`, then the heading `# Sweep the yard`."
        }),
    )
    .await;

    // Git carries no empty directory, so a store with no task yet leaves the worktree without one.
    let worktree = dir.path().join(".git/openplan-rolling-updates/.plan/tasks");
    let count = || {
        worktree
            .read_dir()
            .map_or(0, |entries| entries.flatten().count())
    };
    let before = count();
    let mut stream = Stream::open(&state, &id).await;
    let deadline = std::time::Instant::now() + Duration::from_secs(300);
    loop {
        let event = stream.next().await;
        if event["event"] == "turn_ended" || event["event"] == "exited" {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "the turn never ended");
    }

    assert_eq!(count(), before + 1);
}
