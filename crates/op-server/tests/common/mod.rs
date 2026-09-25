#![allow(dead_code)]

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::response::Response;
use http_body_util::BodyExt;
use op_api::BackendKind;
use op_backend::{Actor, Edit, Op};
use op_server::{AppState, Location, Project, app};
use op_task::{Status, Task};
use op_tracker::Tracker;
use serde_json::Value;
use tower::ServiceExt;

pub const PROJECT: &str = "test";
pub const PATIENCE: Duration = Duration::from_secs(10);
pub const DEFAULT_TAGS: [&str; 3] = ["bug", "draft", "feature"];

pub fn git(dir: &Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .expect("git must be installed for this test");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

pub fn git_output(dir: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .expect("git must be installed for this test");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

pub fn identify(dir: &Path) {
    git(dir, &["config", "user.email", "t@example.com"]);
    git(dir, &["config", "user.name", "Test"]);
}

pub fn repository(dir: &Path) {
    git(dir, &["init", "-q", "-b", "main"]);
    identify(dir);
}

pub fn open(name: &str, dir: &Path, kind: BackendKind) -> Project {
    let location = Location::find(dir, Some(kind)).unwrap();
    Project::open(name, location).unwrap()
}

pub fn started(project: Project, abbreviation: &str) -> Project {
    project
        .tracker()
        .init(project.machine(), abbreviation.parse().unwrap())
        .unwrap();
    project.reload();
    project
}

pub fn local_project(name: &str, dir: &Path, abbreviation: &str) -> Project {
    started(open(name, dir, BackendKind::Local), abbreviation)
}

pub fn git_project(name: &str, dir: &Path, abbreviation: &str) -> Project {
    started(open(name, dir, BackendKind::Git), abbreviation)
}

pub fn local_state() -> (tempfile::TempDir, AppState) {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new([local_project(PROJECT, dir.path(), "OPP")]);
    (dir, state)
}

pub fn git_state() -> (tempfile::TempDir, AppState) {
    let dir = tempfile::tempdir().unwrap();
    repository(dir.path());
    let state = AppState::new([git_project(PROJECT, dir.path(), "OPP")]);
    (dir, state)
}

pub fn project(state: &AppState) -> Arc<Project> {
    state.project(PROJECT).unwrap()
}

// Writes documents as they are, past the tracker's checks, the way a hand edit or an old file can
// hold them.
pub fn seed(state: &AppState, files: &[(&str, &str)]) {
    let project = project(state);
    let ops: Vec<Op> = files
        .iter()
        .map(|(path, text)| Op::put(*path, *text))
        .collect();
    project
        .tracker()
        .backend()
        .commit(project.machine(), &mut |_| {
            Ok(Edit::new("Seed the tasks", ops.clone()))
        })
        .unwrap();
    project.reload();
}

pub fn task_file(status: &str, title: &str, extra: &str) -> String {
    format!("---\nstatus: {status}\ncreated: 2026-01-01T00:00:00Z\n{extra}---\n# {title}\n")
}

// A second handle on the same storage, as another process holds it.
pub fn other_process(project: &Project) -> Tracker {
    let backend = op_server::open_backend(project.location(), &Actor::new("Bob"), false).unwrap();
    Tracker::new(backend)
}

pub fn new_task(title: &str) -> Task {
    Task::new(title, Status::Todo, "2026-01-01T00:00:00Z".parse().unwrap())
}

pub async fn send(state: &AppState, method: &str, uri: &str, body: Option<Value>) -> Response {
    send_as(state, method, uri, body, &[]).await
}

pub async fn send_as(
    state: &AppState,
    method: &str,
    uri: &str,
    body: Option<Value>,
    headers: &[(&str, &str)],
) -> Response {
    let mut builder = Request::builder().method(method).uri(uri);
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    let request = match body {
        Some(value) => builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_vec(&value).unwrap()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    };
    app(state.clone()).oneshot(request).await.unwrap()
}

pub async fn body_json(response: Response) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

pub async fn json_of(state: &AppState, uri: &str) -> Value {
    let response = send(state, "GET", uri, None).await;
    let status = response.status();
    let body = body_json(response).await;
    assert_eq!(status, StatusCode::OK, "GET {uri}: {body}");
    body
}

pub async fn create_in(state: &AppState, project: &str, body: Value) -> String {
    let response = send(
        state,
        "POST",
        &format!("/api/projects/{project}/tasks"),
        Some(body),
    )
    .await;
    let status = response.status();
    let body = body_json(response).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["id"].as_str().unwrap().to_owned()
}

pub async fn create(state: &AppState, title: &str) -> String {
    create_in(state, PROJECT, serde_json::json!({ "title": title })).await
}

pub fn message_of(body: &Value) -> String {
    body["message"].as_str().unwrap().to_owned()
}

pub fn ids(values: &Value) -> Vec<String> {
    values
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value["id"].as_str().unwrap().to_owned())
        .collect()
}

// The pump and the watchdog run on their own threads, so a state a test waits for can come late.
pub async fn until<F, Fut>(mut ready: F)
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
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[derive(Debug, Clone)]
pub struct Sse {
    pub id: Option<String>,
    pub data: Value,
}

// An event ends at a blank line, so only a whole event is parsed, even when it arrives in two
// frames.
pub struct EventStream {
    body: Body,
    buffer: String,
}

impl EventStream {
    pub fn of(response: Response) -> Self {
        assert_eq!(response.status(), StatusCode::OK);
        Self {
            body: response.into_body(),
            buffer: String::new(),
        }
    }

    pub async fn open(state: &AppState, last_event_id: Option<&str>) -> Self {
        let headers: Vec<(&str, &str)> = last_event_id
            .map(|id| ("last-event-id", id))
            .into_iter()
            .collect();
        Self::of(send_as(state, "GET", "/api/events", None, &headers).await)
    }

    pub async fn next(&mut self) -> Option<Sse> {
        loop {
            while let Some(end) = self.buffer.find("\n\n") {
                let event: String = self.buffer.drain(..end + 2).collect();
                let data = event.lines().find_map(|line| line.strip_prefix("data:"));
                let Some(data) = data else { continue };
                let id = event
                    .lines()
                    .find_map(|line| line.strip_prefix("id:"))
                    .map(|id| id.trim().to_owned());
                return Some(Sse {
                    id,
                    data: serde_json::from_str(data.trim()).expect("an event carries JSON"),
                });
            }
            let frame = tokio::time::timeout(PATIENCE, self.body.frame())
                .await
                .expect("the stream sent no frame in time")?
                .unwrap();
            if let Some(data) = frame.data_ref() {
                self.buffer.push_str(&String::from_utf8_lossy(data));
            }
        }
    }

    pub async fn expect(&mut self) -> Sse {
        self.next()
            .await
            .expect("the stream ended before the next event")
    }

    pub async fn find(&mut self, kind: &str) -> Sse {
        loop {
            let event = self
                .next()
                .await
                .unwrap_or_else(|| panic!("the stream ended before a {kind} event"));
            if event.data["kind"] == kind {
                return event;
            }
        }
    }

    pub async fn kinds_to_the_end(mut self) -> Vec<String> {
        let mut kinds = Vec::new();
        while let Some(event) = self.next().await {
            kinds.push(event.data["kind"].as_str().unwrap_or_default().to_owned());
        }
        kinds
    }
}
