use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::response::Response;
use http_body_util::BodyExt;
use op_server::{AppState, app};
use serde_json::{Value, json};
use tower::ServiceExt;

async fn get(uri: &str) -> axum::response::Response {
    let (_dir, state) = store_state();
    app(state)
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap()
}

// A git-backed store whose serve root is checked out on `main` with a birthing commit, so reads
// route through the branch-aware index and writes have a live worktree to land in. This is the
// shape the daemon always serves — a repo is a precondition, not a fallback.
fn store_state() -> (tempfile::TempDir, AppState) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "t@example.com"]);
    git(root, &["config", "user.name", "Test"]);
    std::fs::create_dir_all(root.join(".plan/tasks")).unwrap();
    git(root, &["commit", "-q", "--allow-empty", "-m", "init"]);
    let store = op_store::Store::open(root).unwrap();
    let repo = op_git::Repo::discover(root).unwrap();
    (dir, AppState::new(repo, store))
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

#[tokio::test]
async fn health_returns_ok() {
    let response = get("/health").await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"ok");
}

#[tokio::test]
async fn spa_index_served_with_charset() {
    let response = get("/").await;
    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap();
    assert_eq!(content_type, "text/html; charset=utf-8");
}

#[tokio::test]
async fn health_reports_identity_when_set() {
    let info = op_api::DaemonInfo {
        pid: 4242,
        port: 9,
        version: "9.9.9".to_owned(),
        started_at: 5,
    };
    let (_dir, state) = store_state();
    let response = app(state.with_health(info.clone()))
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let served: op_api::DaemonInfo = serde_json::from_slice(&body).unwrap();
    assert_eq!(served, info);
}

#[tokio::test]
async fn admin_shutdown_returns_ok_with_admin_header() {
    let (_dir, state) = store_state();
    let response = app(state)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/shutdown")
                .header(op_api::ADMIN_HEADER, "1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn admin_shutdown_forbidden_without_admin_header() {
    let (_dir, state) = store_state();
    let response = app(state)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/shutdown")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn tasks_crud_roundtrip() {
    let (_dir, state) = store_state();

    let created = send(
        &state,
        "POST",
        "/api/tasks",
        Some(json!({ "title": "Wire the parser" })),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let id = body_json(created).await["id"].as_str().unwrap().to_owned();
    assert!(id.starts_with("wire-the-parser-"), "slug id: {id}");

    let list = send(&state, "GET", "/api/tasks", None).await;
    assert_eq!(list.status(), StatusCode::OK);
    assert_eq!(body_json(list).await.as_array().unwrap().len(), 1);

    let got = send(&state, "GET", &format!("/api/tasks/{id}"), None).await;
    assert_eq!(got.status(), StatusCode::OK);
    let view = body_json(got).await;
    assert_eq!(view["title"], "Wire the parser");
    assert_eq!(view["status"], "todo");
    assert_eq!(view["body"], "# Wire the parser\n");

    let patched = send(
        &state,
        "PATCH",
        &format!("/api/tasks/{id}"),
        Some(json!({ "status": "in_progress" })),
    )
    .await;
    assert_eq!(patched.status(), StatusCode::OK);
    assert_eq!(body_json(patched).await["status"], "in_progress");

    let deleted = send(&state, "DELETE", &format!("/api/tasks/{id}"), None).await;
    assert_eq!(deleted.status(), StatusCode::NO_CONTENT);

    let gone = send(&state, "GET", &format!("/api/tasks/{id}"), None).await;
    assert_eq!(gone.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn missing_task_routes_are_404() {
    let (_dir, state) = store_state();
    for (method, body) in [
        ("GET", None),
        ("PATCH", Some(json!({ "status": "done" }))),
        ("DELETE", None),
    ] {
        let response = send(&state, method, "/api/tasks/ghost", body).await;
        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "{method} on a missing id must 404"
        );
    }
}

#[tokio::test]
async fn patch_parent_null_clears_absent_leaves_id_sets() {
    let (dir, state) = store_state();
    std::fs::write(
        dir.path().join(".plan/tasks/epic.md"),
        "---\nstatus: todo\n---\n# Epic\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join(".plan/tasks/child.md"),
        "---\nstatus: todo\nparent: epic\n---\n# Child\n",
    )
    .unwrap();

    // Absent key: parent untouched.
    let untouched = send(
        &state,
        "PATCH",
        "/api/tasks/child",
        Some(json!({ "status": "in_progress" })),
    )
    .await;
    assert_eq!(untouched.status(), StatusCode::OK);
    assert_eq!(body_json(untouched).await["parent"], "epic");

    // Explicit null: parent cleared to top level, and the key drops from the file.
    let cleared = send(
        &state,
        "PATCH",
        "/api/tasks/child",
        Some(json!({ "parent": null })),
    )
    .await;
    assert_eq!(cleared.status(), StatusCode::OK);
    assert!(body_json(cleared).await.get("parent").is_none());
    let raw = std::fs::read_to_string(dir.path().join(".plan/tasks/child.md")).unwrap();
    assert!(!raw.contains("parent"), "cleared key must drop: {raw}");

    // Explicit id: parent set again.
    let set = send(
        &state,
        "PATCH",
        "/api/tasks/child",
        Some(json!({ "parent": "epic" })),
    )
    .await;
    assert_eq!(set.status(), StatusCode::OK);
    assert_eq!(body_json(set).await["parent"], "epic");
}

#[tokio::test]
async fn board_groups_by_status_and_nests_same_status_children() {
    let (dir, state) = store_state();
    let tasks = dir.path().join(".plan/tasks");
    std::fs::write(
        tasks.join("epic.md"),
        "---\nstatus: in_progress\n---\n# Epic\n",
    )
    .unwrap();
    std::fs::write(
        tasks.join("sub-open.md"),
        "---\nstatus: in_progress\nparent: epic\nrank: m\n---\n# Sub open\n",
    )
    .unwrap();
    std::fs::write(
        tasks.join("sub-todo.md"),
        "---\nstatus: todo\nparent: epic\n---\n# Sub todo\n",
    )
    .unwrap();

    let board = body_json(send(&state, "GET", "/api/board", None).await).await;
    let groups = board["groups"].as_array().unwrap();
    let order: Vec<&str> = groups
        .iter()
        .map(|g| g["status"].as_str().unwrap())
        .collect();
    assert_eq!(order, vec!["in_progress", "todo"]);

    // Same-status child nests under the epic (depth 1); the todo child surfaces in its own group as
    // a root carrying the parent hint.
    let in_progress = &groups[0]["rows"];
    assert_eq!(in_progress[0]["task"]["id"], "epic");
    assert_eq!(in_progress[0]["depth"], 0);
    assert_eq!(in_progress[0]["has_children"], true);
    assert_eq!(in_progress[1]["task"]["id"], "sub-open");
    assert_eq!(in_progress[1]["depth"], 1);

    let todo = &groups[1]["rows"];
    assert_eq!(todo[0]["task"]["id"], "sub-todo");
    assert_eq!(todo[0]["depth"], 0);
    assert_eq!(todo[0]["parent_title"], "Epic");
}

#[tokio::test]
async fn task_detail_carries_parent_title_children_and_resolved_refs() {
    let (dir, state) = store_state();
    let tasks = dir.path().join(".plan/tasks");
    std::fs::write(tasks.join("epic.md"), "---\nstatus: todo\n---\n# Epic\n").unwrap();
    std::fs::write(
        tasks.join("child.md"),
        "---\nstatus: in_progress\nparent: epic\nrank: m\n---\n# Child\n\nblocks [[epic]], not [[ghost-0000]] or `[[epic]]`.\n",
    )
    .unwrap();
    std::fs::write(
        tasks.join("b.md"),
        "---\nstatus: todo\nparent: child\nrank: t\n---\n# B\n",
    )
    .unwrap();
    std::fs::write(
        tasks.join("a.md"),
        "---\nstatus: todo\nparent: child\nrank: m\n---\n# A\n",
    )
    .unwrap();

    let detail = body_json(send(&state, "GET", "/api/tasks/child", None).await).await;
    assert_eq!(detail["parent_title"], "Epic");

    // Direct children arrive in rank order (a before b), each with title + status.
    let children = detail["children"].as_array().unwrap();
    assert_eq!(
        children
            .iter()
            .map(|c| c["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["a", "b"]
    );
    assert_eq!(children[0]["title"], "A");

    // Only resolvable `[[id]]`s become refs, deduped; a dangling id is dropped.
    let refs = detail["refs"].as_array().unwrap();
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0]["id"], "epic");
    assert_eq!(refs[0]["title"], "Epic");

    // A top-level task reports no parent and no children.
    let epic = body_json(send(&state, "GET", "/api/tasks/epic", None).await).await;
    assert!(epic.get("parent_title").is_none());
    assert_eq!(epic["children"][0]["id"], "child");
}

#[tokio::test]
async fn patch_preserves_unknown_frontmatter_keys() {
    let (dir, state) = store_state();
    std::fs::write(
        dir.path().join(".plan/tasks/keep.md"),
        "---\nstatus: todo\nestimate: 9\n---\n# Keep\n",
    )
    .unwrap();

    let response = send(
        &state,
        "PATCH",
        "/api/tasks/keep",
        Some(json!({ "status": "done" })),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let raw = std::fs::read_to_string(dir.path().join(".plan/tasks/keep.md")).unwrap();
    assert!(
        raw.contains("estimate: 9"),
        "estimate must survive PATCH: {raw}"
    );
    assert!(raw.contains("status: done"));
}

#[tokio::test]
async fn create_with_unknown_parent_is_400() {
    let (_dir, state) = store_state();
    let response = send(
        &state,
        "POST",
        "/api/tasks",
        Some(json!({ "title": "Child", "parent": "ghost" })),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn malformed_json_body_is_400() {
    let (_dir, state) = store_state();
    let request = Request::builder()
        .method("POST")
        .uri("/api/tasks")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{ not json"))
        .unwrap();
    let response = app(state).oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn serve_stops_on_external_shutdown() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let (_dir, state) = store_state();
    let server = tokio::spawn(op_server::serve(listener, state, async move {
        let _ = rx.await;
    }));

    tx.send(()).unwrap();
    server.await.unwrap().unwrap();
}

#[tokio::test]
async fn events_endpoint_is_an_event_stream() {
    let response = get("/api/events").await;
    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap();
    assert_eq!(content_type, "text/event-stream");
}

#[tokio::test]
async fn events_stream_delivers_published_changes() {
    let (_dir, state) = store_state();

    // The GET resolves once the handler has subscribed, so a change published afterwards is
    // buffered for this receiver rather than lost.
    let events = send(&state, "GET", "/api/events", None).await;
    assert_eq!(events.status(), StatusCode::OK);

    let created = send(
        &state,
        "POST",
        "/api/tasks",
        Some(json!({ "title": "Wire the SSE" })),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let id = body_json(created).await["id"].as_str().unwrap().to_owned();

    let event: Value = serde_json::from_str(&first_sse_data(events).await).unwrap();
    assert_eq!(event["kind"], "task_changed");
    assert_eq!(event["id"], id);
}

#[tokio::test]
async fn events_stream_ends_on_shutdown_with_final_event() {
    let (_dir, state) = store_state();

    // The GET resolves once the handler has subscribed to the shutdown watch, so the stop
    // triggered afterwards reaches this open stream.
    let events = send(&state, "GET", "/api/events", None).await;
    assert_eq!(events.status(), StatusCode::OK);

    let shutdown = app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/shutdown")
                .header(op_api::ADMIN_HEADER, "1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(shutdown.status(), StatusCode::OK);

    let kinds = tokio::time::timeout(std::time::Duration::from_secs(5), drain_sse_kinds(events))
        .await
        .expect("event stream must end after the daemon signals shutdown");
    assert_eq!(kinds.last().map(String::as_str), Some("daemon_stopping"));
}

#[tokio::test]
async fn events_stream_frees_subscription_when_client_disconnects() {
    let (_dir, state) = store_state();
    let baseline = state.event_sender().receiver_count();

    let events = send(&state, "GET", "/api/events", None).await;
    assert_eq!(events.status(), StatusCode::OK);
    assert_eq!(state.event_sender().receiver_count(), baseline + 1);

    // The client goes away without another event ever being published.
    drop(events);

    assert!(
        wait_for_receiver_count(&state, baseline).await,
        "the SSE pump task kept its broadcast subscription alive after the client disconnected"
    );
}

#[tokio::test]
async fn events_stream_ends_on_shutdown_even_when_send_buffer_is_full() {
    let (_dir, state) = store_state();
    let baseline = state.event_sender().receiver_count();

    // The client connects but never reads its body, so the handler's mpsc buffer fills.
    let _events = send(&state, "GET", "/api/events", None).await;
    assert_eq!(state.event_sender().receiver_count(), baseline + 1);

    // Overflow the handler far past its channel capacity so the pump task parks inside
    // `tx.send().await` on the full, un-drained mpsc.
    for _ in 0..2048 {
        let _ = state.event_sender().send(op_api::ChangeEvent::RefMoved {
            branch: String::new(),
        });
    }
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert_eq!(state.event_sender().receiver_count(), baseline + 1);

    // The daemon stops. A stream blocked on a full send must still tear down promptly instead
    // of pinning graceful shutdown open until the stop deadline.
    let shutdown = app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/shutdown")
                .header(op_api::ADMIN_HEADER, "1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(shutdown.status(), StatusCode::OK);

    assert!(
        wait_for_receiver_count(&state, baseline).await,
        "a stream blocked on a full send did not observe shutdown and release its subscription"
    );
}

async fn wait_for_receiver_count(state: &AppState, target: usize) -> bool {
    for _ in 0..200 {
        if state.event_sender().receiver_count() == target {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    state.event_sender().receiver_count() == target
}

async fn drain_sse_kinds(response: Response) -> Vec<String> {
    let mut body = response.into_body();
    let mut buffer = String::new();
    let mut kinds = Vec::new();
    while let Some(frame) = body.frame().await {
        if let Some(data) = frame.unwrap().data_ref() {
            buffer.push_str(&String::from_utf8_lossy(data));
        }
        while let Some(end) = buffer.find("\n\n") {
            let event: String = buffer.drain(..end + 2).collect();
            let Some(line) = event.lines().find_map(|line| line.strip_prefix("data:")) else {
                continue;
            };
            if let Ok(value) = serde_json::from_str::<Value>(line.trim()) {
                if let Some(kind) = value["kind"].as_str() {
                    kinds.push(kind.to_owned());
                }
            }
        }
    }
    kinds
}

async fn first_sse_data(response: Response) -> String {
    let mut body = response.into_body();
    let mut buffer = String::new();
    while let Some(frame) = body.frame().await {
        if let Some(data) = frame.unwrap().data_ref() {
            buffer.push_str(&String::from_utf8_lossy(data));
        }
        // An SSE event ends at a blank line; only parse a fully-received event so a payload
        // split across frames is never read half-formed.
        while let Some(end) = buffer.find("\n\n") {
            let event: String = buffer.drain(..end + 2).collect();
            if let Some(line) = event.lines().find_map(|line| line.strip_prefix("data:")) {
                return line.trim().to_owned();
            }
        }
    }
    panic!("event stream closed before delivering a data frame");
}

fn git(dir: &std::path::Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .current_dir(dir)
        .args(args)
        .status()
        .expect("git must be installed for this test");
    assert!(status.success(), "git {args:?} failed");
}

// A git-backed store whose `alpha` task is `todo` on the checked-out main branch and `done` on a
// non-checked-out feature branch, which edited it later — so latest-change-wins headlines `done`.
// Commit dates are explicit so the ordering never hinges on same-second wall-clock ties.
fn git_state() -> (tempfile::TempDir, AppState) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "t@example.com"]);
    git(root, &["config", "user.name", "Test"]);
    std::fs::create_dir_all(root.join(".plan/tasks")).unwrap();
    write_alpha(root, "todo", "Alpha");
    git(root, &["add", "."]);
    git_commit_at(root, 1_000_000_000, "init");
    git(root, &["checkout", "-q", "-b", "feature"]);
    write_alpha(root, "done", "Alpha done");
    git_commit_at(root, 1_000_000_100, "edit alpha");
    git(root, &["checkout", "-q", "main"]);

    let store = op_store::Store::open(root).unwrap();
    let repo = op_git::Repo::discover(root).unwrap();
    (dir, AppState::new(repo, store))
}

#[tokio::test]
async fn list_tasks_is_branch_aware() {
    let (_dir, state) = git_state();
    let response = send(&state, "GET", "/api/tasks", None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    let items = body.as_array().unwrap();

    // One row per logical task, even though `alpha` lives on two branches.
    assert_eq!(items.len(), 1, "one entry per logical task: {items:?}");
    let alpha = &items[0];
    assert_eq!(alpha["id"], "alpha");
    // Headline follows the most recently changed branch: `feature` edited alpha after main's init.
    assert_eq!(alpha["status"], "done");
    assert_eq!(alpha["title"], "Alpha done");
    assert_eq!(
        alpha["headline"], "feature",
        "the row names its headline branch"
    );

    let branches = alpha["branches"].as_array().unwrap();
    assert_eq!(branches.len(), 2, "carries both branches: {branches:?}");
    let names: Vec<&str> = branches
        .iter()
        .map(|b| b["branch"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["feature", "main"], "sorted by branch name");
    let feature = branches.iter().find(|b| b["branch"] == "feature").unwrap();
    assert_eq!(feature["status"], "done", "feature's own version");
}

#[tokio::test]
async fn cross_branch_task_read_reflects_the_other_branch() {
    let (_dir, state) = git_state();
    let response = send(&state, "GET", "/api/tasks/alpha?branch=feature", None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let view = body_json(response).await;
    assert_eq!(view["status"], "done");
    assert_eq!(view["title"], "Alpha done");

    // Omitting the branch headlines the most recently changed version, which here is feature's.
    let local = send(&state, "GET", "/api/tasks/alpha", None).await;
    assert_eq!(body_json(local).await["status"], "done");
}

#[tokio::test]
async fn cross_branch_read_missing_id_or_branch_is_404() {
    let (_dir, state) = git_state();
    let missing_branch = send(&state, "GET", "/api/tasks/alpha?branch=ghost", None).await;
    assert_eq!(missing_branch.status(), StatusCode::NOT_FOUND);
    let missing_id = send(&state, "GET", "/api/tasks/ghost?branch=feature", None).await;
    assert_eq!(missing_id.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn branchless_get_carries_the_branch_set() {
    let (_dir, state) = git_state();
    let response = send(&state, "GET", "/api/tasks/alpha", None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let view = body_json(response).await;
    // Headline is the most recently changed version (feature), flattened alongside the branch set.
    assert_eq!(view["status"], "done");
    assert_eq!(view["title"], "Alpha done");
    assert_eq!(
        view["headline"], "feature",
        "names the branch the headline resolves to"
    );
    let names: Vec<&str> = view["branches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["branch"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["feature", "main"], "every branch it lives on");
}

#[tokio::test]
async fn write_to_a_branch_without_a_live_worktree_is_refused() {
    let (_dir, state) = git_state();
    // `feature` exists but is not checked out in any worktree, so the server cannot write to it
    // without fabricating a commit — which it refuses to do.
    let patch = send(
        &state,
        "PATCH",
        "/api/tasks/alpha?branch=feature",
        Some(json!({ "status": "done" })),
    )
    .await;
    assert_eq!(patch.status(), StatusCode::CONFLICT);

    let delete = send(&state, "DELETE", "/api/tasks/alpha?branch=feature", None).await;
    assert_eq!(delete.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn delete_is_local_to_the_in_view_branch() {
    let (_dir, state) = git_state();
    // Delete on the current (main) worktree removes only main's copy; `feature` still carries it,
    // so the task survives in the list.
    let deleted = send(&state, "DELETE", "/api/tasks/alpha", None).await;
    assert_eq!(deleted.status(), StatusCode::NO_CONTENT);

    let list = send(&state, "GET", "/api/tasks", None).await;
    let items = body_json(list).await;
    let alpha = items
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == "alpha")
        .expect("alpha survives on feature");
    let names: Vec<&str> = alpha["branches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["branch"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["feature"], "only feature retains alpha");
}

// Like `git_state`, but `feature` is checked out in a live linked worktree so writes to it land,
// and its `alpha` diverges to `done` over main's `todo`.
fn git_state_live_feature() -> (tempfile::TempDir, AppState) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "t@example.com"]);
    git(root, &["config", "user.name", "Test"]);
    std::fs::create_dir_all(root.join(".plan/tasks")).unwrap();
    std::fs::write(
        root.join(".plan/tasks/alpha.md"),
        "---\nstatus: todo\n---\n# Alpha\n",
    )
    .unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "init"]);

    let wt = root.join(".worktrees/feature");
    git(
        root,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "feature",
            wt.to_str().unwrap(),
        ],
    );
    std::fs::write(
        wt.join(".plan/tasks/alpha.md"),
        "---\nstatus: done\n---\n# Alpha\n",
    )
    .unwrap();
    git(&wt, &["commit", "-qam", "feature: alpha done"]);

    let store = op_store::Store::open(root).unwrap();
    let repo = op_git::Repo::discover(root).unwrap();
    (dir, AppState::new(repo, store))
}

#[tokio::test]
async fn patch_reverting_a_branch_to_its_base_echoes_the_written_task() {
    let (_dir, state) = git_state_live_feature();
    // feature's alpha is `done`; main's base is `todo`. Reverting feature back to `todo` makes it
    // match the base, so feature no longer diverges and drops out of the matrix — the write must
    // still succeed and echo the task, not 404.
    let patch = send(
        &state,
        "PATCH",
        "/api/tasks/alpha?branch=feature",
        Some(json!({ "status": "todo" })),
    )
    .await;
    assert_eq!(patch.status(), StatusCode::OK);
    let view = body_json(patch).await;
    assert_eq!(view["id"], "alpha");
    assert_eq!(view["status"], "todo");
    assert_eq!(view["title"], "Alpha");
}

#[tokio::test]
async fn branchless_get_of_a_task_dropped_everywhere_live_still_loads() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "t@example.com"]);
    git(root, &["config", "user.name", "Test"]);
    std::fs::create_dir_all(root.join(".plan/tasks")).unwrap();
    std::fs::write(
        root.join(".plan/tasks/alpha.md"),
        "---\nstatus: todo\n---\n# Alpha\n",
    )
    .unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "init"]);
    git(root, &["checkout", "-q", "-b", "feature"]);
    std::fs::remove_file(root.join(".plan/tasks/alpha.md")).unwrap();
    git(root, &["commit", "-qam", "feature: drop alpha"]);
    git(root, &["checkout", "-q", "main"]);
    // main's working tree also drops it (uncommitted), so no live branch carries alpha — its only
    // matrix cell is feature's committed deletion.
    std::fs::remove_file(root.join(".plan/tasks/alpha.md")).unwrap();

    let store = op_store::Store::open(root).unwrap();
    let repo = op_git::Repo::discover(root).unwrap();
    let state = AppState::new(repo, store);

    let list = send(&state, "GET", "/api/tasks", None).await;
    let items = body_json(list).await;
    assert!(
        items.as_array().unwrap().iter().any(|i| i["id"] == "alpha"),
        "the pending deletion still lists: {items}"
    );

    // A task the list still shows must open, not 404: the branchless headline falls back to the
    // deletion's last-known blob.
    let response = send(&state, "GET", "/api/tasks/alpha", None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let view = body_json(response).await;
    assert_eq!(view["title"], "Alpha");
}

fn write_alpha(root: &std::path::Path, status: &str, title: &str) {
    std::fs::write(
        root.join(".plan/tasks/alpha.md"),
        format!("---\nstatus: {status}\n---\n# {title}\n"),
    )
    .unwrap();
}

fn git_commit_at(root: &std::path::Path, secs: i64, msg: &str) {
    let date = format!("@{secs} +0000");
    let status = std::process::Command::new("git")
        .current_dir(root)
        .args(["commit", "-qam", msg])
        .env("GIT_AUTHOR_DATE", &date)
        .env("GIT_COMMITTER_DATE", &date)
        .status()
        .expect("git must be installed for this test");
    assert!(status.success(), "git commit failed");
}

#[tokio::test]
async fn headline_follows_the_most_recent_change_even_on_the_default_branch() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "t@example.com"]);
    git(root, &["config", "user.name", "Test"]);
    std::fs::create_dir_all(root.join(".plan/tasks")).unwrap();

    // main creates alpha=todo (t0); feature diverges it to in_progress (t1); then main advances it
    // to done (t2), the newest change of all. Latest-change-wins must headline main's done over
    // feature's older in_progress — proving the default branch competes on time, and isn't ignored
    // just because a feature branch also diverged. (git_state proves the reverse direction.)
    write_alpha(root, "todo", "Alpha");
    git(root, &["add", "."]);
    git_commit_at(root, 1_000_000_000, "init");

    git(root, &["checkout", "-q", "-b", "feature"]);
    write_alpha(root, "in_progress", "Alpha wip");
    git_commit_at(root, 1_000_000_100, "feature wip");

    git(root, &["checkout", "-q", "main"]);
    write_alpha(root, "done", "Alpha done");
    git_commit_at(root, 1_000_000_200, "main done");

    let store = op_store::Store::open(root).unwrap();
    let repo = op_git::Repo::discover(root).unwrap();
    let state = AppState::new(repo, store);

    let list = send(&state, "GET", "/api/tasks", None).await;
    let alpha = body_json(list).await;
    let alpha = alpha
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["id"] == "alpha")
        .unwrap()
        .clone();
    assert_eq!(
        alpha["status"], "done",
        "main's newer change headlines over feature's older one"
    );
    assert_eq!(alpha["title"], "Alpha done");

    let detail = send(&state, "GET", "/api/tasks/alpha", None).await;
    assert_eq!(body_json(detail).await["status"], "done");
}

#[tokio::test]
async fn openapi_spec_is_served_over_http() {
    let response = get("/api-docs/openapi.json").await;
    assert_eq!(response.status(), StatusCode::OK);
    let spec = body_json(response).await;
    assert_eq!(spec["info"]["title"], "open-planner");
    assert!(spec["paths"].get("/api/tasks").is_some());
    assert!(spec["paths"].get("/api/tasks/{id}").is_some());
}

#[tokio::test]
async fn swagger_ui_page_is_served() {
    let response = get("/swagger-ui/").await;
    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    assert!(content_type.starts_with("text/html"));
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let html = String::from_utf8_lossy(&bytes);
    assert!(html.to_lowercase().contains("swagger"));
}
