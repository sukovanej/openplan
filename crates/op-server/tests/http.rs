use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::response::Response;
use http_body_util::BodyExt;
use op_server::{AppState, Project, app};
use serde_json::{Value, json};
use tower::ServiceExt;

const PROJECT: &str = "test";

fn project_state(
    root: impl AsRef<std::path::Path>,
    repo: op_git::Repo,
    store: op_store::Store,
) -> AppState {
    let root = root.as_ref().to_path_buf();
    // Some of these stores are opened by hand, past the `.plan/config.toml` a discovery would read.
    let config = op_store::Config {
        abbreviation: store.abbreviation(),
        default_branch: None,
    };
    AppState::new([Project::new(PROJECT, root, repo, store, &config)])
}

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
    std::fs::write(root.join(".plan/config.toml"), "abbreviation = \"OPP\"\n").unwrap();
    git(root, &["commit", "-q", "--allow-empty", "-m", "init"]);
    let store = op_store::Store::discover(root).unwrap();
    let repo = op_git::Repo::discover(root).unwrap();
    let state = project_state(root, repo, store);
    (dir, state)
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

// A worktree added mid-test carries no uncommitted `config.toml`, so it is opened the way the index
// opens a sibling worktree: under the serving store's abbreviation.
fn worktree_store(path: impl AsRef<std::path::Path>) -> op_store::Store {
    op_store::Store::open(path, "OPP".parse().unwrap()).unwrap()
}

// A test that reaches past the API into the store crosses from the key spelling to the number that
// names a file.
fn number(key: &str) -> u64 {
    key.strip_prefix("OPP-").unwrap().parse().unwrap()
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
        "/api/projects/test/tasks",
        Some(json!({ "title": "Wire the parser" })),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let id = body_json(created).await["id"].as_str().unwrap().to_owned();
    assert_eq!(
        id, "OPP-1",
        "the id is the store's key for the allocated number"
    );

    let list = send(&state, "GET", "/api/projects/test/tasks", None).await;
    assert_eq!(list.status(), StatusCode::OK);
    assert_eq!(body_json(list).await.as_array().unwrap().len(), 1);

    let got = send(
        &state,
        "GET",
        &format!("/api/projects/test/tasks/{id}"),
        None,
    )
    .await;
    assert_eq!(got.status(), StatusCode::OK);
    let view = body_json(got).await;
    assert_eq!(view["title"], "Wire the parser");
    assert_eq!(view["metadata"]["status"], "backlog");
    assert_eq!(view["body"], "# Wire the parser\n");

    let patched = send(
        &state,
        "PATCH",
        &format!("/api/projects/test/tasks/{id}"),
        Some(json!({ "status": "in_progress" })),
    )
    .await;
    assert_eq!(patched.status(), StatusCode::OK);
    assert_eq!(
        body_json(patched).await["metadata"]["status"],
        "in_progress"
    );

    let deleted = send(
        &state,
        "DELETE",
        &format!("/api/projects/test/tasks/{id}"),
        None,
    )
    .await;
    assert_eq!(deleted.status(), StatusCode::NO_CONTENT);

    let gone = send(
        &state,
        "GET",
        &format!("/api/projects/test/tasks/{id}"),
        None,
    )
    .await;
    assert_eq!(gone.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn search_matches_the_body_and_carries_the_branch() {
    let (_dir, state) = store_state();
    send(
        &state,
        "POST",
        "/api/projects/test/tasks",
        Some(json!({ "title": "Wire the parser", "body": "It must accept a zeppelin." })),
    )
    .await;
    send(
        &state,
        "POST",
        "/api/projects/test/tasks",
        Some(json!({ "title": "Paint the shed" })),
    )
    .await;

    for uri in [
        "/api/projects/test/search?q=ZEPPELIN",
        "/api/search?q=zeppelin",
    ] {
        let response = send(&state, "GET", uri, None).await;
        assert_eq!(response.status(), StatusCode::OK, "{uri}");
        let hits = body_json(response).await;
        let hits = hits.as_array().unwrap();
        assert_eq!(hits.len(), 1, "{uri}");
        assert_eq!(hits[0]["task"]["id"], "OPP-1", "{uri}");
        assert_eq!(hits[0]["branch"], "main", "{uri}");
    }
}

#[tokio::test]
async fn search_with_no_query_matches_nothing() {
    let (_dir, state) = store_state();
    send(
        &state,
        "POST",
        "/api/projects/test/tasks",
        Some(json!({ "title": "Wire the parser" })),
    )
    .await;

    for uri in [
        "/api/projects/test/search",
        "/api/projects/test/search?q=",
        "/api/search?q=",
    ] {
        let response = send(&state, "GET", uri, None).await;
        assert_eq!(response.status(), StatusCode::OK, "{uri}");
        assert!(
            body_json(response).await.as_array().unwrap().is_empty(),
            "{uri}"
        );
    }
}

#[tokio::test]
async fn searching_an_unknown_project_is_404() {
    let (_dir, state) = store_state();
    let response = send(&state, "GET", "/api/projects/nope/search?q=a", None).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// A path segment no id could ever name is a bad request, not a missing task — and it must read the
// same whichever method asks, though only the write routes reach the store.
#[tokio::test]
async fn routes_naming_something_that_is_not_an_id_are_400() {
    let (_dir, state) = store_state();
    for (method, body) in [
        ("GET", None),
        ("PATCH", Some(json!({ "status": "done" }))),
        ("DELETE", None),
    ] {
        let response = send(
            &state,
            method,
            "/api/projects/test/tasks/ship-login-3d0c",
            body,
        )
        .await;
        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "{method} on a non-id must be a bad request"
        );
    }
}

#[tokio::test]
async fn missing_task_routes_are_404() {
    let (_dir, state) = store_state();
    for (method, body) in [
        ("GET", None),
        ("PATCH", Some(json!({ "status": "done" }))),
        ("DELETE", None),
    ] {
        let response = send(&state, method, "/api/projects/test/tasks/OPP-99", body).await;
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
        dir.path().join(".plan/tasks/00001-epic.md"),
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Epic\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join(".plan/tasks/00002-child.md"),
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nparent: '1'\n---\n# Child\n",
    )
    .unwrap();

    // Absent key: parent untouched.
    let untouched = send(
        &state,
        "PATCH",
        "/api/projects/test/tasks/OPP-2",
        Some(json!({ "status": "in_progress" })),
    )
    .await;
    assert_eq!(untouched.status(), StatusCode::OK);
    assert_eq!(body_json(untouched).await["metadata"]["parent"], "OPP-1");

    // Explicit null: parent cleared to top level, and the key drops from the file.
    let cleared = send(
        &state,
        "PATCH",
        "/api/projects/test/tasks/OPP-2",
        Some(json!({ "parent": null })),
    )
    .await;
    assert_eq!(cleared.status(), StatusCode::OK);
    assert!(body_json(cleared).await.get("parent").is_none());
    let raw = std::fs::read_to_string(dir.path().join(".plan/tasks/00002-child.md")).unwrap();
    assert!(!raw.contains("parent"), "cleared key must drop: {raw}");

    // Explicit id: parent set again.
    let set = send(
        &state,
        "PATCH",
        "/api/projects/test/tasks/OPP-2",
        Some(json!({ "parent": "OPP-1" })),
    )
    .await;
    assert_eq!(set.status(), StatusCode::OK);
    assert_eq!(body_json(set).await["metadata"]["parent"], "OPP-1");
}

#[tokio::test]
async fn board_groups_by_status_and_nests_same_status_children() {
    let (dir, state) = store_state();
    let tasks = dir.path().join(".plan/tasks");
    std::fs::write(
        tasks.join("00001-epic.md"),
        "---\nstatus: in_progress\ncreated: 2026-01-01T00:00:00Z\n---\n# Epic\n",
    )
    .unwrap();
    std::fs::write(
        tasks.join("00002-sub-open.md"),
        "---\nstatus: in_progress\ncreated: 2026-01-01T00:00:00Z\nparent: '1'\nrank: m\n---\n# Sub open\n",
    )
    .unwrap();
    std::fs::write(
        tasks.join("00003-sub-todo.md"),
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nparent: '1'\n---\n# Sub todo\n",
    )
    .unwrap();

    let board = body_json(send(&state, "GET", "/api/projects/test/board", None).await).await;
    let groups = board["groups"].as_array().unwrap();
    let order: Vec<&str> = groups
        .iter()
        .map(|g| g["status"].as_str().unwrap())
        .collect();
    assert_eq!(order, vec!["in_progress", "todo"]);

    // Same-status child nests under the epic (depth 1); the todo child surfaces in its own group as
    // a root carrying the parent hint.
    let in_progress = &groups[0]["rows"];
    assert_eq!(in_progress[0]["task"]["id"], "OPP-1");
    assert_eq!(in_progress[0]["depth"], 0);
    assert_eq!(in_progress[0]["has_children"], true);
    assert_eq!(in_progress[1]["task"]["id"], "OPP-2");
    assert_eq!(in_progress[1]["depth"], 1);

    let todo = &groups[1]["rows"];
    assert_eq!(todo[0]["task"]["id"], "OPP-3");
    assert_eq!(todo[0]["depth"], 0);
    assert_eq!(todo[0]["parent_title"], "Epic");
}

#[tokio::test]
async fn task_detail_carries_parent_title_children_and_resolved_refs() {
    let (dir, state) = store_state();
    let tasks = dir.path().join(".plan/tasks");
    std::fs::write(
        tasks.join("00001-epic.md"),
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Epic\n",
    )
    .unwrap();
    std::fs::write(
        tasks.join("00002-child.md"),
        "---\nstatus: in_progress\ncreated: 2026-01-01T00:00:00Z\nparent: '1'\nrank: m\n---\n# Child\n\nblocks [[./00001-epic.md]] and [[OPP-1]], not [[OPP-99]] or [[1]].\n",
    )
    .unwrap();
    std::fs::write(
        tasks.join("00004-b.md"),
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nparent: '2'\nrank: t\n---\n# B\n",
    )
    .unwrap();
    std::fs::write(
        tasks.join("00003-a.md"),
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nparent: '2'\nrank: m\n---\n# A\n",
    )
    .unwrap();

    let detail = body_json(send(&state, "GET", "/api/projects/test/tasks/OPP-2", None).await).await;
    assert_eq!(detail["parent_title"], "Epic");

    // Direct children arrive in rank order (a before b), each with title + status.
    let children = detail["children"].as_array().unwrap();
    assert_eq!(
        children
            .iter()
            .map(|c| c["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["OPP-3", "OPP-4"]
    );
    assert_eq!(children[0]["title"], "A");

    // Only a resolvable reference becomes a ref, deduped across both its spellings; a dangling key
    // and a bare number are dropped.
    let refs = detail["refs"].as_array().unwrap();
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0]["id"], "OPP-1");
    assert_eq!(refs[0]["title"], "Epic");

    // A top-level task reports no parent and no children.
    let epic = body_json(send(&state, "GET", "/api/projects/test/tasks/OPP-1", None).await).await;
    assert!(epic.get("parent_title").is_none());
    assert_eq!(epic["children"][0]["id"], "OPP-2");
}

#[tokio::test]
async fn task_detail_carries_both_directions_of_its_dependencies() {
    let (dir, state) = store_state();
    let tasks = dir.path().join(".plan/tasks");
    std::fs::write(
        tasks.join("00001-design.md"),
        "---\nstatus: done\ncreated: 2026-01-01T00:00:00Z\n---\n# Design\n",
    )
    .unwrap();
    // A plain entry, two that name two sections of one task, and one no file holds.
    std::fs::write(
        tasks.join("00002-api.md"),
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\ndependencies:\n- ./00003-schema.md\n- ./00001-design.md#Wire\n- ./00001-design.md#Shape\n- ./00099-gone.md\n---\n# API\n",
    )
    .unwrap();
    std::fs::write(
        tasks.join("00003-schema.md"),
        "---\nstatus: done\ncreated: 2026-01-01T00:00:00Z\n---\n# Schema\n",
    )
    .unwrap();
    std::fs::write(
        tasks.join("00004-ship.md"),
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nrank: t\ndependencies:\n- ./00002-api.md\n---\n# Ship\n",
    )
    .unwrap();
    std::fs::write(
        tasks.join("00005-web.md"),
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nrank: m\ndependencies:\n- ./00002-api.md#Wire\n---\n# Web\n",
    )
    .unwrap();

    let detail = body_json(send(&state, "GET", "/api/projects/test/tasks/OPP-2", None).await).await;
    let depends_on = detail["depends_on"].as_array().unwrap();
    assert_eq!(
        depends_on
            .iter()
            .map(|d| d["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["OPP-3", "OPP-1"],
        "file order; an entry that names no task drops out, and one task counts once"
    );
    assert_eq!(depends_on[0]["title"], "Schema");
    assert_eq!(depends_on[0]["status"], "done");

    let blocks = detail["blocks"].as_array().unwrap();
    assert_eq!(
        blocks
            .iter()
            .map(|b| b["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["OPP-5", "OPP-4"],
        "rank order, and a sectioned entry still reports the task it names"
    );

    // A task that waits for nothing ships no `depends_on` at all.
    let schema = body_json(send(&state, "GET", "/api/projects/test/tasks/OPP-3", None).await).await;
    assert!(schema.get("depends_on").is_none());
    assert_eq!(schema["blocks"][0]["id"], "OPP-2");

    // The write path builds its own detail, so it has to carry both fields too.
    let patched = body_json(
        send(
            &state,
            "PATCH",
            "/api/projects/test/tasks/OPP-2",
            Some(json!({ "status": "in_progress" })),
        )
        .await,
    )
    .await;
    assert_eq!(patched["depends_on"][0]["id"], "OPP-3");
    assert_eq!(patched["blocks"][0]["id"], "OPP-5");
}

#[tokio::test]
async fn patch_preserves_unknown_frontmatter_keys() {
    let (dir, state) = store_state();
    std::fs::write(
        dir.path().join(".plan/tasks/00001-keep.md"),
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nestimate: 9\n---\n# Keep\n",
    )
    .unwrap();

    let response = send(
        &state,
        "PATCH",
        "/api/projects/test/tasks/OPP-1",
        Some(json!({ "status": "done" })),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let raw = std::fs::read_to_string(dir.path().join(".plan/tasks/00001-keep.md")).unwrap();
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
        "/api/projects/test/tasks",
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
        .uri("/api/projects/test/tasks")
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
        "/api/projects/test/tasks",
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
            project: PROJECT.to_owned(),
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

    std::fs::write(root.join(".plan/config.toml"), "abbreviation = \"OPP\"\n").unwrap();
    let store = op_store::Store::discover(root).unwrap();
    let repo = op_git::Repo::discover(root).unwrap();
    let state = project_state(root, repo, store);
    (dir, state)
}

#[tokio::test]
async fn list_tasks_is_branch_aware() {
    let (_dir, state) = git_state();
    let response = send(&state, "GET", "/api/projects/test/tasks", None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    let items = body.as_array().unwrap();

    // One row per logical task, even though `alpha` lives on two branches.
    assert_eq!(items.len(), 1, "one entry per logical task: {items:?}");
    let alpha = &items[0];
    assert_eq!(alpha["id"], "OPP-1");
    // Headline follows the most recently changed branch: `feature` edited alpha after main's init.
    assert_eq!(alpha["metadata"]["status"], "done");
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
    let response = send(
        &state,
        "GET",
        "/api/projects/test/tasks/OPP-1?branch=feature",
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let view = body_json(response).await;
    assert_eq!(view["metadata"]["status"], "done");
    assert_eq!(view["title"], "Alpha done");

    // Omitting the branch headlines the most recently changed version, which here is feature's.
    let local = send(&state, "GET", "/api/projects/test/tasks/OPP-1", None).await;
    assert_eq!(body_json(local).await["metadata"]["status"], "done");
}

#[tokio::test]
async fn cross_branch_read_missing_id_or_branch_is_404() {
    let (_dir, state) = git_state();
    let missing_branch = send(
        &state,
        "GET",
        "/api/projects/test/tasks/OPP-1?branch=ghost",
        None,
    )
    .await;
    assert_eq!(missing_branch.status(), StatusCode::NOT_FOUND);
    let missing_id = send(
        &state,
        "GET",
        "/api/projects/test/tasks/OPP-99?branch=feature",
        None,
    )
    .await;
    assert_eq!(missing_id.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn branchless_get_carries_the_branch_set() {
    let (_dir, state) = git_state();
    let response = send(&state, "GET", "/api/projects/test/tasks/OPP-1", None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let view = body_json(response).await;
    // Headline is the most recently changed version (feature), flattened alongside the branch set.
    assert_eq!(view["metadata"]["status"], "done");
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
        "/api/projects/test/tasks/OPP-1?branch=feature",
        Some(json!({ "status": "done" })),
    )
    .await;
    assert_eq!(patch.status(), StatusCode::CONFLICT);

    let delete = send(
        &state,
        "DELETE",
        "/api/projects/test/tasks/OPP-1?branch=feature",
        None,
    )
    .await;
    assert_eq!(delete.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn delete_is_local_to_the_in_view_branch() {
    let (_dir, state) = git_state();
    // Delete on the current (main) worktree removes only main's copy; `feature` still carries it,
    // so the task survives in the list.
    let deleted = send(&state, "DELETE", "/api/projects/test/tasks/OPP-1", None).await;
    assert_eq!(deleted.status(), StatusCode::NO_CONTENT);

    let list = send(&state, "GET", "/api/projects/test/tasks", None).await;
    let items = body_json(list).await;
    let alpha = items
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == "OPP-1")
        .expect("alpha survives on feature");
    let names: Vec<&str> = alpha["branches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["branch"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["feature"], "only feature retains alpha");
}

// A task the serve root's branch never carried: `feature` adds `alpha` in its own live worktree,
// and `main` has no copy of it at all. The aggregated reads still list it, so a write to it has to
// land somewhere.
fn git_state_alpha_only_on_live_feature() -> (tempfile::TempDir, AppState) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "t@example.com"]);
    git(root, &["config", "user.name", "Test"]);
    std::fs::create_dir_all(root.join(".plan/tasks")).unwrap();
    std::fs::write(root.join(".plan/config.toml"), "abbreviation = \"OPP\"\n").unwrap();
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
    std::fs::create_dir_all(wt.join(".plan/tasks")).unwrap();
    write_alpha(&wt, "todo", "Alpha");
    git(&wt, &["add", "."]);
    git(&wt, &["commit", "-qm", "feature: add alpha"]);

    let store = op_store::Store::discover(root).unwrap();
    let repo = op_git::Repo::discover(root).unwrap();
    let state = project_state(root, repo, store);
    (dir, state)
}

// Like the above, but no worktree holds `feature`, so nothing can write `alpha` at all.
fn git_state_alpha_only_on_a_parked_branch() -> (tempfile::TempDir, AppState) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "t@example.com"]);
    git(root, &["config", "user.name", "Test"]);
    std::fs::create_dir_all(root.join(".plan/tasks")).unwrap();
    std::fs::write(root.join(".plan/config.toml"), "abbreviation = \"OPP\"\n").unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "init"]);
    git(root, &["checkout", "-q", "-b", "feature"]);
    write_alpha(root, "todo", "Alpha");
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "feature: add alpha"]);
    git(root, &["checkout", "-q", "main"]);

    let store = op_store::Store::discover(root).unwrap();
    let repo = op_git::Repo::discover(root).unwrap();
    let state = project_state(root, repo, store);
    (dir, state)
}

fn row(items: &Value, id: &str) -> Value {
    items
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == id)
        .unwrap_or_else(|| panic!("{id} is listed: {items:?}"))
        .clone()
}

fn message_of(body: &Value) -> String {
    body["message"].as_str().unwrap().to_owned()
}

#[tokio::test]
async fn a_read_names_the_branch_a_write_would_land_on() {
    let (_dir, state) = git_state_alpha_only_on_live_feature();
    let list = send(&state, "GET", "/api/projects/test/tasks", None).await;
    let alpha = row(&body_json(list).await, "OPP-1");
    assert_eq!(
        alpha["write_target"],
        json!({ "branch": "feature", "writable": true }),
        "main never carried alpha, so a write goes to feature's live worktree"
    );

    let detail = send(&state, "GET", "/api/projects/test/tasks/OPP-1", None).await;
    assert_eq!(
        body_json(detail).await["write_target"],
        json!({ "branch": "feature", "writable": true })
    );
}

// A read scoped to a branch shows that branch's version, so it must name that branch as what an edit
// of what is on screen writes to. Naming the branch a branchless write would pick instead would have
// the client edit a version it is not showing.
#[tokio::test]
async fn a_branch_scoped_read_writes_to_the_branch_it_read() {
    let (_dir, state) = git_state_live_feature();
    let detail = send(
        &state,
        "GET",
        "/api/projects/test/tasks/OPP-1?branch=feature",
        None,
    )
    .await;
    let view = body_json(detail).await;
    assert_eq!(view["metadata"]["status"], "done", "feature's version");
    assert_eq!(
        view["write_target"],
        json!({ "branch": "feature", "writable": true }),
        "the version on screen is feature's, and so is the write"
    );

    let list = send(&state, "GET", "/api/projects/test/tasks?branch=main", None).await;
    assert_eq!(
        row(&body_json(list).await, "OPP-1")["write_target"],
        json!({ "branch": "main", "writable": true }),
        "a branch-scoped list reads and writes the same branch"
    );
}

// A task a branch agrees with its merge-base about contributes no matrix cell, so it can be
// headlined by no branch at all while a live worktree still holds the file. The write has to find
// that worktree — this is the 404 [[OPP-35]] is about, in the shape the matrix cannot see.
#[tokio::test]
async fn a_task_with_no_matrix_cell_still_writes_where_it_lives() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "t@example.com"]);
    git(root, &["config", "user.name", "Test"]);
    std::fs::create_dir_all(root.join(".plan/tasks")).unwrap();
    std::fs::write(root.join(".plan/config.toml"), "abbreviation = \"OPP\"\n").unwrap();
    write_alpha(root, "todo", "Alpha");
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "init"]);

    // `feature` keeps alpha exactly as the merge-base has it, so it diverges in nothing and gets no
    // cell; main then drops the task, which leaves the task with no cell anywhere.
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
    std::fs::remove_file(root.join(".plan/tasks/00001-alpha.md")).unwrap();
    git(root, &["commit", "-qam", "main: drop alpha"]);

    let store = op_store::Store::discover(root).unwrap();
    let repo = op_git::Repo::discover(root).unwrap();
    let state = project_state(root, repo, store);

    let list = send(
        &state,
        "GET",
        "/api/projects/test/tasks?branch=feature",
        None,
    )
    .await;
    assert_eq!(
        row(&body_json(list).await, "OPP-1")["write_target"],
        json!({ "branch": "feature", "writable": true })
    );

    let patch = send(
        &state,
        "PATCH",
        "/api/projects/test/tasks/OPP-1",
        Some(json!({ "status": "done" })),
    )
    .await;
    assert_eq!(patch.status(), StatusCode::OK, "the file is right there");
    assert!(
        worktree_store(&wt)
            .read_raw(1)
            .unwrap()
            .contains("status: done"),
        "and the write lands in the worktree that holds it"
    );
}

#[tokio::test]
async fn patch_writes_through_to_the_branch_that_holds_the_task() {
    let (dir, state) = git_state_alpha_only_on_live_feature();
    let patch = send(
        &state,
        "PATCH",
        "/api/projects/test/tasks/OPP-1",
        Some(json!({ "status": "done" })),
    )
    .await;
    assert_eq!(patch.status(), StatusCode::OK);
    assert_eq!(body_json(patch).await["metadata"]["status"], "done");

    let feature = worktree_store(dir.path().join(".worktrees/feature"));
    assert!(
        feature.read_raw(1).unwrap().contains("status: done"),
        "the write lands in the worktree that holds the task"
    );
    assert!(
        !worktree_store(dir.path()).exists(1),
        "and never conjures a copy in the serve root"
    );
}

#[tokio::test]
async fn delete_writes_through_to_the_branch_that_holds_the_task() {
    let (dir, state) = git_state_alpha_only_on_live_feature();
    let deleted = send(&state, "DELETE", "/api/projects/test/tasks/OPP-1", None).await;
    assert_eq!(deleted.status(), StatusCode::NO_CONTENT);
    assert!(
        !worktree_store(dir.path().join(".worktrees/feature")).exists(1),
        "feature's copy is the one that goes"
    );
}

#[tokio::test]
async fn a_named_branch_that_lacks_the_task_is_refused_with_where_it_lives() {
    let (_dir, state) = git_state_alpha_only_on_live_feature();
    let patch = send(
        &state,
        "PATCH",
        "/api/projects/test/tasks/OPP-1?branch=main",
        Some(json!({ "status": "done" })),
    )
    .await;
    assert_eq!(patch.status(), StatusCode::NOT_FOUND);
    let message = message_of(&body_json(patch).await);
    assert!(
        message.contains("branch main") && message.contains("it lives on feature"),
        "the refusal names the branch it wrote and the branch that holds it: {message}"
    );
}

#[tokio::test]
async fn a_task_no_branch_holds_is_refused_as_no_such_task() {
    let (_dir, state) = git_state_alpha_only_on_live_feature();
    let patch = send(
        &state,
        "PATCH",
        "/api/projects/test/tasks/OPP-99",
        Some(json!({ "status": "done" })),
    )
    .await;
    assert_eq!(patch.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        message_of(&body_json(patch).await),
        "no such task: OPP-99",
        "nothing to point at, so the refusal stays plain"
    );
}

#[tokio::test]
async fn a_task_no_live_worktree_holds_reads_as_unwritable() {
    let (_dir, state) = git_state_alpha_only_on_a_parked_branch();
    let list = send(&state, "GET", "/api/projects/test/tasks", None).await;
    let alpha = row(&body_json(list).await, "OPP-1");
    assert_eq!(
        alpha["write_target"],
        json!({ "branch": "feature", "writable": false }),
        "the write would go to feature, and no worktree holds it: {alpha:?}"
    );

    let patch = send(
        &state,
        "PATCH",
        "/api/projects/test/tasks/OPP-1",
        Some(json!({ "status": "done" })),
    )
    .await;
    assert_eq!(patch.status(), StatusCode::CONFLICT);
    let message = message_of(&body_json(patch).await);
    assert!(
        message.contains("feature"),
        "the refusal names the branch it would have written: {message}"
    );
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
        root.join(".plan/tasks/00001-alpha.md"),
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Alpha\n",
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
        wt.join(".plan/tasks/00001-alpha.md"),
        "---\nstatus: done\ncreated: 2026-01-01T00:00:00Z\n---\n# Alpha\n",
    )
    .unwrap();
    git(&wt, &["commit", "-qam", "feature: alpha done"]);

    std::fs::write(root.join(".plan/config.toml"), "abbreviation = \"OPP\"\n").unwrap();
    let store = op_store::Store::discover(root).unwrap();
    let repo = op_git::Repo::discover(root).unwrap();
    let state = project_state(root, repo, store);
    (dir, state)
}

#[tokio::test]
async fn a_stalled_serve_root_refuses_rather_than_redirecting_the_write() {
    let (dir, state) = git_state_live_feature();
    // Main carries alpha, so the write is main's to make — and main is mid-merge, so it cannot be
    // made at all. `feature` carries alpha too, and holding the write is the only honest answer:
    // landing it there would edit a version nobody asked about.
    std::fs::write(dir.path().join(".git/MERGE_HEAD"), "").unwrap();
    let patch = send(
        &state,
        "PATCH",
        "/api/projects/test/tasks/OPP-1",
        Some(json!({ "status": "in_progress" })),
    )
    .await;
    assert_eq!(patch.status(), StatusCode::CONFLICT);
    let message = message_of(&body_json(patch).await);
    assert!(
        message.contains("main"),
        "the refusal names the branch that owns the write: {message}"
    );
    let detail = send(&state, "GET", "/api/projects/test/tasks/OPP-1", None).await;
    assert_eq!(
        body_json(detail).await["write_target"],
        json!({ "branch": "main", "writable": false }),
        "and the read names main too, not the branch it headlines on"
    );
    assert!(
        worktree_store(dir.path().join(".worktrees/feature"))
            .read_raw(1)
            .unwrap()
            .contains("status: done"),
        "feature's version stands untouched"
    );
}

#[tokio::test]
async fn create_lands_in_the_requested_branch_s_own_worktree() {
    let (dir, state) = git_state_live_feature();
    let created = send(
        &state,
        "POST",
        "/api/projects/test/tasks?branch=feature",
        Some(json!({ "title": "Ship login" })),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let id = body_json(created).await["id"].as_str().unwrap().to_owned();

    assert!(
        worktree_store(dir.path().join(".worktrees/feature")).exists(number(&id)),
        "the new task belongs to feature's worktree"
    );
    assert!(
        !worktree_store(dir.path()).exists(number(&id)),
        "creation is branch-local: main's worktree is untouched"
    );
}

#[tokio::test]
async fn create_on_a_branch_without_a_live_worktree_is_refused() {
    let (dir, state) = git_state();
    // `feature` exists but no worktree holds it, so there is nowhere to write without fabricating a
    // commit — the same refusal patch and delete make.
    let created = send(
        &state,
        "POST",
        "/api/projects/test/tasks?branch=feature",
        Some(json!({ "title": "Ship login" })),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CONFLICT);
    assert_eq!(
        std::fs::read_dir(dir.path().join(".plan/tasks"))
            .unwrap()
            .count(),
        1,
        "a refused create must not fall back to the serve root"
    );
}

#[tokio::test]
async fn create_numbers_ids_above_every_local_branch() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "t@example.com"]);
    git(root, &["config", "user.name", "Test"]);
    std::fs::create_dir_all(root.join(".plan/tasks")).unwrap();
    std::fs::write(
        root.join(".plan/tasks/00002-alpha-2.md"),
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Alpha\n",
    )
    .unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "init"]);
    // The highest number in the repo sits on a branch that is not checked out. An allocator that
    // only looked at the working tree would hand out 3 and collide with it on the next merge.
    git(root, &["checkout", "-q", "-b", "feature"]);
    std::fs::write(
        root.join(".plan/tasks/00009-beta-9.md"),
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Beta\n",
    )
    .unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "beta on feature"]);
    git(root, &["checkout", "-q", "main"]);

    std::fs::write(root.join(".plan/config.toml"), "abbreviation = \"OPP\"\n").unwrap();
    let store = op_store::Store::discover(root).unwrap();
    let repo = op_git::Repo::discover(root).unwrap();
    let state = project_state(root, repo, store);

    for expected in ["OPP-10", "OPP-11"] {
        let created = send(
            &state,
            "POST",
            "/api/projects/test/tasks",
            Some(json!({ "title": "Ship login" })),
        )
        .await;
        assert_eq!(created.status(), StatusCode::CREATED);
        assert_eq!(body_json(created).await["id"], expected);
    }
}

#[tokio::test]
async fn create_never_reuses_a_number_a_branch_still_holds() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "t@example.com"]);
    git(root, &["config", "user.name", "Test"]);
    std::fs::create_dir_all(root.join(".plan/tasks")).unwrap();
    std::fs::write(
        root.join(".plan/tasks/00005-alpha-5.md"),
        "---\nstatus: todo\n---\n# Alpha\n",
    )
    .unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "init"]);
    // `feature` keeps alpha-5 exactly as the merge base has it, so it never shows up as divergence;
    // main then drops the task. The number is still in use — on a branch the matrix has no cell for.
    git(root, &["branch", "feature"]);
    std::fs::remove_file(root.join(".plan/tasks/00005-alpha-5.md")).unwrap();
    git(root, &["commit", "-qam", "drop alpha"]);

    std::fs::write(root.join(".plan/config.toml"), "abbreviation = \"OPP\"\n").unwrap();
    let store = op_store::Store::discover(root).unwrap();
    let repo = op_git::Repo::discover(root).unwrap();
    let state = project_state(root, repo, store);

    let created = send(
        &state,
        "POST",
        "/api/projects/test/tasks",
        Some(json!({ "title": "Beta" })),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    assert_eq!(
        body_json(created).await["id"],
        "OPP-6",
        "reissuing 5 would put two unrelated tasks under one id once feature merges"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn concurrent_creates_never_share_an_id() {
    let (_dir, state) = store_state();
    // Same title, so the slug cannot tell the ids apart — only the allocated number can. Each
    // request rebuilds the index and reads the same floor, so a per-request `max + 1` would collide.
    let mut requests = Vec::new();
    for _ in 0..8 {
        let state = state.clone();
        requests.push(tokio::spawn(async move {
            let created = send(
                &state,
                "POST",
                "/api/projects/test/tasks",
                Some(json!({ "title": "Contended" })),
            )
            .await;
            assert_eq!(created.status(), StatusCode::CREATED);
            body_json(created).await["id"].as_str().unwrap().to_owned()
        }));
    }

    let mut ids = Vec::new();
    for request in requests {
        ids.push(request.await.unwrap());
    }
    let distinct: std::collections::HashSet<&String> = ids.iter().collect();
    assert_eq!(distinct.len(), ids.len(), "ids collided: {ids:?}");

    let list = send(&state, "GET", "/api/projects/test/tasks", None).await;
    assert_eq!(
        body_json(list).await.as_array().unwrap().len(),
        ids.len(),
        "every create landed as its own task"
    );
}

#[tokio::test]
async fn create_takes_the_next_number_when_one_is_already_on_disk() {
    let (dir, state) = store_state();
    // A file written outside the daemon can hold the number the floor just cleared; the create takes
    // the next one instead of failing.
    std::fs::write(
        dir.path().join(".plan/tasks/00001-contended-1.md"),
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Contended\n",
    )
    .unwrap();

    let created = send(
        &state,
        "POST",
        "/api/projects/test/tasks",
        Some(json!({ "title": "Contended" })),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    assert_eq!(body_json(created).await["id"], "OPP-2");
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
        "/api/projects/test/tasks/OPP-1?branch=feature",
        Some(json!({ "status": "todo" })),
    )
    .await;
    assert_eq!(patch.status(), StatusCode::OK);
    let view = body_json(patch).await;
    assert_eq!(view["id"], "OPP-1");
    assert_eq!(view["metadata"]["status"], "todo");
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
        root.join(".plan/tasks/00001-alpha.md"),
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Alpha\n",
    )
    .unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "init"]);
    git(root, &["checkout", "-q", "-b", "feature"]);
    std::fs::remove_file(root.join(".plan/tasks/00001-alpha.md")).unwrap();
    git(root, &["commit", "-qam", "feature: drop alpha"]);
    git(root, &["checkout", "-q", "main"]);
    // main's working tree also drops it (uncommitted), so no live branch carries alpha — its only
    // matrix cell is feature's committed deletion.
    std::fs::remove_file(root.join(".plan/tasks/00001-alpha.md")).unwrap();

    std::fs::write(root.join(".plan/config.toml"), "abbreviation = \"OPP\"\n").unwrap();
    let store = op_store::Store::discover(root).unwrap();
    let repo = op_git::Repo::discover(root).unwrap();
    let state = project_state(root, repo, store);

    let list = send(&state, "GET", "/api/projects/test/tasks", None).await;
    let items = body_json(list).await;
    assert!(
        items.as_array().unwrap().iter().any(|i| i["id"] == "OPP-1"),
        "the pending deletion still lists: {items}"
    );

    // A task the list still shows must open, not 404: the branchless headline falls back to the
    // deletion's last-known blob.
    let response = send(&state, "GET", "/api/projects/test/tasks/OPP-1", None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let view = body_json(response).await;
    assert_eq!(view["title"], "Alpha");
}

fn write_alpha(root: &std::path::Path, status: &str, title: &str) {
    std::fs::write(
        root.join(".plan/tasks/00001-alpha.md"),
        format!("---\nstatus: {status}\ncreated: 2026-01-01T00:00:00Z\n---\n# {title}\n"),
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

    std::fs::write(root.join(".plan/config.toml"), "abbreviation = \"OPP\"\n").unwrap();
    let store = op_store::Store::discover(root).unwrap();
    let repo = op_git::Repo::discover(root).unwrap();
    let state = project_state(root, repo, store);

    let list = send(&state, "GET", "/api/projects/test/tasks", None).await;
    let alpha = body_json(list).await;
    let alpha = alpha
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["id"] == "OPP-1")
        .unwrap()
        .clone();
    assert_eq!(
        alpha["metadata"]["status"], "done",
        "main's newer change headlines over feature's older one"
    );
    assert_eq!(alpha["title"], "Alpha done");

    let detail = send(&state, "GET", "/api/projects/test/tasks/OPP-1", None).await;
    assert_eq!(body_json(detail).await["metadata"]["status"], "done");
}

#[tokio::test]
async fn openapi_spec_is_served_over_http() {
    let response = get("/api-docs/openapi.json").await;
    assert_eq!(response.status(), StatusCode::OK);
    let spec = body_json(response).await;
    assert_eq!(spec["info"]["title"], "openplan");
    assert!(spec["paths"].get("/api/projects/{project}/tasks").is_some());
    assert!(
        spec["paths"]
            .get("/api/projects/{project}/tasks/{id}")
            .is_some()
    );
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

#[tokio::test]
async fn patch_rejects_a_malformed_rank_with_its_reason() {
    let (dir, state) = store_state();
    std::fs::write(
        dir.path().join(".plan/tasks/00001-solo.md"),
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Solo\n",
    )
    .unwrap();

    let refused = send(
        &state,
        "PATCH",
        "/api/projects/test/tasks/OPP-1",
        Some(json!({ "rank": "NOT-BASE36" })),
    )
    .await;
    assert_eq!(refused.status(), StatusCode::BAD_REQUEST);
    let message = body_json(refused).await["message"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(message.contains("rank"), "message: {message}");
    let raw = std::fs::read_to_string(dir.path().join(".plan/tasks/00001-solo.md")).unwrap();
    assert!(!raw.contains("rank"), "a refused rank must not land: {raw}");

    let accepted = send(
        &state,
        "PATCH",
        "/api/projects/test/tasks/OPP-1",
        Some(json!({ "rank": "a5" })),
    )
    .await;
    assert_eq!(accepted.status(), StatusCode::OK);
    assert_eq!(body_json(accepted).await["metadata"]["rank"], "a5");
}

#[tokio::test]
async fn patching_a_parent_that_would_cycle_is_refused_with_its_reason() {
    // The web pickers exclude cycle-forming targets from a snapshot that can be stale by the time
    // the write lands, so this rejection is reachable through the UI and must carry a usable reason.
    let (dir, state) = store_state();
    std::fs::write(
        dir.path().join(".plan/tasks/00001-epic.md"),
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Epic\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join(".plan/tasks/00002-child.md"),
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nparent: '1'\n---\n# Child\n",
    )
    .unwrap();

    let refused = send(
        &state,
        "PATCH",
        "/api/projects/test/tasks/OPP-1",
        Some(json!({ "parent": "OPP-2" })),
    )
    .await;
    assert_eq!(refused.status(), StatusCode::BAD_REQUEST);
    let message = body_json(refused).await["message"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(message.contains("descendant"), "message: {message}");
}

// The unprefixed task spellings are gone with the SPA that called them. A caller that still uses
// one must be told so, rather than handed the SPA's index.html by the static fallback.
#[tokio::test]
async fn the_unprefixed_task_spellings_are_gone() {
    let (_dir, state) = store_state();
    for uri in ["/api/config", "/api/tasks", "/api/tasks/OPP-1"] {
        let response = send(&state, "GET", uri, None).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{uri}");
        let message = body_json(response).await["message"]
            .as_str()
            .unwrap()
            .to_owned();
        assert!(message.contains(uri), "{uri} must name itself: {message}");
    }
}

// The merged board is a route of its own rather than a delegation, so it outlives them.
#[tokio::test]
async fn the_unprefixed_board_still_answers() {
    let (_dir, state) = store_state();
    assert_eq!(
        send(&state, "GET", "/api/board", None).await.status(),
        StatusCode::OK
    );
}

#[tokio::test]
async fn an_unknown_project_is_404_that_names_the_registered_ones() {
    let (_dir, state) = store_state();
    for uri in [
        "/api/projects/ghost/tasks",
        "/api/projects/ghost/board",
        "/api/projects/ghost/tasks/OPP-1",
    ] {
        let response = send(&state, "GET", uri, None).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{uri}");
        let message = body_json(response).await["message"]
            .as_str()
            .unwrap()
            .to_owned();
        assert!(message.contains("ghost"), "{uri}: {message}");
        assert!(message.contains(PROJECT), "{uri}: {message}");
    }
}

fn two_project_state() -> (tempfile::TempDir, tempfile::TempDir, AppState) {
    let (alpha_dir, alpha) = one_project("alpha");
    let (beta_dir, beta) = one_project("beta");
    (alpha_dir, beta_dir, AppState::new([alpha, beta]))
}

fn one_project(name: &str) -> (tempfile::TempDir, Project) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "t@example.com"]);
    git(root, &["config", "user.name", "Test"]);
    std::fs::create_dir_all(root.join(".plan/tasks")).unwrap();
    std::fs::write(root.join(".plan/config.toml"), "abbreviation = \"OPP\"\n").unwrap();
    git(root, &["commit", "-q", "--allow-empty", "-m", "init"]);
    let store = op_store::Store::discover(root).unwrap();
    let repo = op_git::Repo::discover(root).unwrap();
    let config = op_store::Config::read(root).unwrap();
    let project = Project::new(name, root.to_path_buf(), repo, store, &config);
    (dir, project)
}

// Each project counts its own ids, so the same number naming a task in both is correct, not a
// collision: a key is store-scoped and the project is the coordinate that tells them apart.
#[tokio::test]
async fn two_projects_write_to_their_own_store_under_their_own_numbers() {
    let (alpha_dir, beta_dir, state) = two_project_state();

    for (project, dir, title) in [
        ("alpha", &alpha_dir, "Alpha one"),
        ("beta", &beta_dir, "Beta one"),
    ] {
        let created = send(
            &state,
            "POST",
            &format!("/api/projects/{project}/tasks"),
            Some(json!({ "title": title })),
        )
        .await;
        assert_eq!(created.status(), StatusCode::CREATED);
        assert_eq!(body_json(created).await["id"], "OPP-1");

        let detail = send(
            &state,
            "GET",
            &format!("/api/projects/{project}/tasks/OPP-1"),
            None,
        )
        .await;
        assert_eq!(body_json(detail).await["title"], title);
        assert_eq!(
            std::fs::read_dir(dir.path().join(".plan/tasks"))
                .unwrap()
                .count(),
            1,
            "{project}: each write lands in its own store only"
        );
    }
}

#[test]
fn the_openapi_spec_documents_every_json_api_route() {
    let spec = serde_json::to_value(op_server::openapi()).unwrap();
    let paths = spec["paths"].as_object().unwrap();
    for route in [
        "/api/projects/{project}/tasks",
        "/api/projects/{project}/tasks/{id}",
        "/api/projects/{project}/board",
        "/api/board",
        "/api/flow",
        "/health",
    ] {
        assert!(paths.contains_key(route), "{route} missing from the spec");
    }
    for schema in ["Board", "Flow"] {
        assert!(
            spec["components"]["schemas"].get(schema).is_some(),
            "{schema} must reach the spec the web client is generated from"
        );
    }
}

// Every refusal a caller can act on must be documented with its `ApiErrorBody`: the generated web
// client turns exactly these into typed errors carrying the server's reason, and an undocumented
// status reaches the UI as a bare status code instead.
#[test]
fn the_openapi_spec_documents_every_refusal_with_its_reason() {
    let spec = serde_json::to_value(op_server::openapi()).unwrap();
    for (method, route, status) in [
        ("get", "/api/projects/{project}/tasks", "400"),
        ("get", "/api/projects/{project}/tasks", "404"),
        ("get", "/api/projects/{project}/tasks", "500"),
        ("post", "/api/projects/{project}/tasks", "400"),
        ("post", "/api/projects/{project}/tasks", "404"),
        ("post", "/api/projects/{project}/tasks", "409"),
        ("post", "/api/projects/{project}/tasks", "500"),
        ("get", "/api/board", "500"),
        ("get", "/api/flow", "400"),
        ("get", "/api/flow", "404"),
        ("get", "/api/flow", "422"),
        ("get", "/api/flow", "500"),
        ("get", "/api/flow", "503"),
        ("get", "/api/projects/{project}/board", "400"),
        ("get", "/api/projects/{project}/board", "404"),
        ("get", "/api/projects/{project}/board", "500"),
        ("get", "/api/projects/{project}/tasks/{id}", "400"),
        ("get", "/api/projects/{project}/tasks/{id}", "404"),
        ("get", "/api/projects/{project}/tasks/{id}", "500"),
        ("patch", "/api/projects/{project}/tasks/{id}", "400"),
        ("patch", "/api/projects/{project}/tasks/{id}", "404"),
        ("patch", "/api/projects/{project}/tasks/{id}", "409"),
        ("patch", "/api/projects/{project}/tasks/{id}", "500"),
        ("delete", "/api/projects/{project}/tasks/{id}", "400"),
        ("delete", "/api/projects/{project}/tasks/{id}", "404"),
        ("delete", "/api/projects/{project}/tasks/{id}", "409"),
        ("delete", "/api/projects/{project}/tasks/{id}", "500"),
    ] {
        let schema = &spec["paths"][route][method]["responses"][status]["content"]["application/json"]
            ["schema"];
        assert_eq!(
            schema["$ref"], "#/components/schemas/ApiErrorBody",
            "{method} {route} -> {status} must document its reason body"
        );
    }
}

// The merged board carries no project segment because it answers over every project at once, so
// every row has to name its own. A client reading it deep-links a task from the row alone.
#[test]
fn the_spec_documents_the_merged_board_and_the_project_every_task_names() {
    let spec = serde_json::to_value(op_server::openapi()).unwrap();
    assert_eq!(
        spec["paths"]["/api/board"]["get"]["responses"]["200"]["content"]["application/json"]["schema"]
            ["$ref"],
        "#/components/schemas/Board"
    );
    assert!(
        spec["paths"]["/api/board"]["get"]["parameters"].is_null(),
        "the merged board takes no project"
    );
    let schemas = &spec["components"]["schemas"];
    for schema in ["TaskListItem", "TaskDetail"] {
        assert_eq!(
            schemas[schema]["properties"]["project"]["type"], "string",
            "{schema}.project must be a plain string"
        );
        assert!(
            schemas[schema]["required"]
                .as_array()
                .unwrap()
                .iter()
                .any(|field| field == "project"),
            "{schema}.project must always be present"
        );
    }
}

// A field the server skips when empty is absent, never null, so the spec must not widen it to
// nullable — that would push an impossible `| null` into every generated client type. A frontmatter
// field is a different case: it is always present, as a value or as an error, and `null` there is a
// real value (no parent), so it is exempt.
#[test]
fn optional_response_fields_are_absent_rather_than_nullable() {
    let spec = serde_json::to_value(op_server::openapi()).unwrap();
    let schemas = &spec["components"]["schemas"];
    for (schema, field) in [
        ("TaskChild", "rank"),
        ("BoardRow", "parent_title"),
        ("TaskDetail", "parent_title"),
    ] {
        assert_eq!(
            schemas[schema]["properties"][field]["type"], "string",
            "{schema}.{field} must be a plain optional string"
        );
    }
}

#[tokio::test]
async fn a_restart_never_reissues_a_number_only_a_file_holds() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "t@example.com"]);
    git(root, &["config", "user.name", "Test"]);
    std::fs::create_dir_all(root.join(".plan/tasks")).unwrap();
    // No commit yet, so nothing the branch walk can see holds these numbers — only the files do.
    std::fs::write(root.join(".plan/config.toml"), "abbreviation = \"OPP\"\n").unwrap();
    let store = op_store::Store::discover(root).unwrap();
    let repo = op_git::Repo::discover(root).unwrap();
    let state = project_state(root, repo.clone(), store.clone());
    for title in ["Alpha", "Beta"] {
        let created = send(
            &state,
            "POST",
            "/api/projects/test/tasks",
            Some(json!({ "title": title })),
        )
        .await;
        assert_eq!(created.status(), StatusCode::CREATED);
    }

    // A fresh state is a restarted daemon: the counter is gone and the floor must come off disk.
    let restarted = project_state(root, repo, store);
    let created = send(
        &restarted,
        "POST",
        "/api/projects/test/tasks",
        Some(json!({ "title": "Gamma" })),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    assert_eq!(body_json(created).await["id"], "OPP-3");
}

#[tokio::test]
async fn create_refuses_once_the_highest_number_is_taken() {
    let (dir, state) = store_state();
    let taken = format!(".plan/tasks/{}-alpha.md", u64::MAX);
    std::fs::write(
        dir.path().join(taken),
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Alpha\n",
    )
    .unwrap();

    // Reissuing the top number would put two unrelated tasks under one. Refusing is the only answer
    // that keeps a number naming one task.
    let created = send(
        &state,
        "POST",
        "/api/projects/test/tasks",
        Some(json!({ "title": "Beta" })),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CONFLICT);
    assert!(
        body_json(created).await["message"]
            .as_str()
            .unwrap()
            .contains("no number is left to issue")
    );
}

#[tokio::test]
async fn a_write_names_the_vanished_root_rather_than_the_branch() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "t@example.com"]);
    git(root, &["config", "user.name", "Test"]);
    std::fs::create_dir_all(root.join(".plan/tasks")).unwrap();
    std::fs::write(
        root.join(".plan/tasks/00001-alpha-1.md"),
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Alpha\n",
    )
    .unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "init"]);
    let feature = root.join("wt");
    git(
        root,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "feature",
            feature.to_str().unwrap(),
        ],
    );

    let feature_store = worktree_store(&feature);
    let state = project_state(
        &feature,
        op_git::Repo::discover(&feature).unwrap(),
        feature_store,
    );
    git(
        root,
        &["worktree", "remove", "--force", feature.to_str().unwrap()],
    );

    // Every branch reads as unwritable once the root is gone, so blaming the branch would send the
    // user to inspect a checkout that is fine.
    let created = send(
        &state,
        "POST",
        "/api/projects/test/tasks?branch=main",
        Some(json!({ "title": "Beta" })),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CONFLICT);
    let message = body_json(created).await["message"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(message.contains("no longer exists"), "{message}");
    assert!(message.contains("openplan server restart"), "{message}");
}

// A patch applies field by field and stops at the first bad key, so a write that reports 400 must
// have changed nothing — the file would otherwise hold a status the client was told did not land.
#[tokio::test]
async fn a_patch_refused_for_a_bad_key_writes_nothing() {
    let (dir, state) = store_state();
    let path = dir.path().join(".plan/tasks/00001-alpha.md");
    let before = "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Alpha\n";
    std::fs::write(&path, before).unwrap();

    let refused = send(
        &state,
        "PATCH",
        "/api/projects/test/tasks/OPP-1",
        Some(json!({ "status": "done", "parent": "42" })),
    )
    .await;

    assert_eq!(refused.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        before,
        "a refused patch must leave the file alone"
    );
}

// The echo a write returns has to read like the read path: the index resolves a parent by key, so
// handing it the number the frontmatter carries would report every task as parentless.
#[tokio::test]
async fn a_patch_echoes_the_parent_title() {
    let (dir, state) = store_state();
    let tasks = dir.path().join(".plan/tasks");
    std::fs::write(
        tasks.join("00001-epic.md"),
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Epic\n",
    )
    .unwrap();
    std::fs::write(
        tasks.join("00002-child.md"),
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nparent: './00001-epic.md'\n---\n# Child\n",
    )
    .unwrap();

    let patched = send(
        &state,
        "PATCH",
        "/api/projects/test/tasks/OPP-2",
        Some(json!({ "status": "done" })),
    )
    .await;

    assert_eq!(patched.status(), StatusCode::OK);
    let detail = body_json(patched).await;
    assert_eq!(detail["metadata"]["parent"], "OPP-1");
    assert_eq!(detail["parent_title"], "Epic");
}

#[tokio::test]
async fn a_branch_scoped_list_answers_for_that_branch_alone() {
    let (_dir, state) = git_state();
    let response = send(
        &state,
        "GET",
        "/api/projects/test/tasks?branch=feature",
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let items = body_json(response).await;
    let items = items.as_array().unwrap();

    // Same shape as the aggregated form, so one client decodes both — but every field describes the
    // branch that was asked about, not the one that headlines the task.
    assert_eq!(items.len(), 1, "{items:?}");
    assert_eq!(items[0]["id"], "OPP-1");
    assert_eq!(items[0]["metadata"]["status"], "done");
    assert_eq!(items[0]["headline"], "feature");
    let branches = items[0]["branches"].as_array().unwrap();
    assert_eq!(branches.len(), 1, "only its own state: {branches:?}");
    assert_eq!(branches[0]["branch"], "feature");

    let main = send(&state, "GET", "/api/projects/test/tasks?branch=main", None).await;
    let main = body_json(main).await;
    assert_eq!(main[0]["metadata"]["status"], "todo", "main's own version");
}

// An empty answer would read as "this branch has no tasks", which is a different fact and one the
// caller could act on. A branch the repository does not have is the caller's mistake.
#[tokio::test]
async fn an_unknown_branch_is_refused_rather_than_answered_empty() {
    let (_dir, state) = git_state();
    let response = send(&state, "GET", "/api/projects/test/tasks?branch=ghost", None).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        body_json(response).await["message"],
        "no such branch: ghost"
    );
}

#[tokio::test]
async fn the_matrix_route_carries_one_cell_per_task_branch() {
    let (_dir, state) = git_state();
    let response = send(&state, "GET", "/api/projects/test/matrix", None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let cells = body_json(response).await;
    let cells = cells["cells"].as_array().unwrap().clone();
    assert_eq!(
        cells.len(),
        2,
        "alpha on main + alpha on feature: {cells:?}"
    );
    let branches: Vec<&str> = cells
        .iter()
        .map(|cell| cell["branch"].as_str().unwrap())
        .collect();
    assert_eq!(branches, vec!["feature", "main"]);
}

#[tokio::test]
async fn the_branches_route_groups_the_versions() {
    let (_dir, state) = git_state();
    let response = send(
        &state,
        "GET",
        "/api/projects/test/tasks/OPP-1/branches",
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let view = body_json(response).await;
    assert_eq!(view["id"], "OPP-1");
    assert_eq!(
        view["versions"].as_array().unwrap().len(),
        2,
        "the two branches disagree: {view}"
    );

    let missing = send(
        &state,
        "GET",
        "/api/projects/test/tasks/OPP-99/branches",
        None,
    )
    .await;
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_tree_route_walks_the_branchs_own_task_set() {
    let (dir, state) = store_state();
    let tasks = dir.path().join(".plan/tasks");
    std::fs::write(
        tasks.join("00001-root.md"),
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Root\n",
    )
    .unwrap();
    std::fs::write(
        tasks.join("00002-child.md"),
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nparent: './00001-root.md'\n---\n# Child\n",
    )
    .unwrap();
    std::fs::write(
        tasks.join("00003-grandchild.md"),
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nparent: './00002-child.md'\n---\n# Grandchild\n",
    )
    .unwrap();

    let response = send(&state, "GET", "/api/projects/test/tasks/OPP-1/tree", None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let view = body_json(response).await;
    assert_eq!(view["tree"]["id"], "OPP-1");
    assert_eq!(view["tree"]["children"][0]["id"], "OPP-2");
    assert_eq!(view["tree"]["children"][0]["children"][0]["id"], "OPP-3");

    let bounded = send(
        &state,
        "GET",
        "/api/projects/test/tasks/OPP-1/tree?depth=1",
        None,
    )
    .await;
    let bounded = body_json(bounded).await;
    assert!(
        bounded["tree"]["children"][0]["children"]
            .as_array()
            .unwrap()
            .is_empty(),
        "depth 1 stops at the direct children: {bounded}"
    );

    let missing = send(&state, "GET", "/api/projects/test/tasks/OPP-99/tree", None).await;
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
}

// A parent cycle has no bottom to walk to. The subtree is truncated there, and the response says
// where, so a client does not render a truncated hierarchy as a complete one.
#[tokio::test]
async fn the_tree_route_reports_a_truncated_cycle() {
    let (dir, state) = store_state();
    let tasks = dir.path().join(".plan/tasks");
    std::fs::write(
        tasks.join("00001-a.md"),
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nparent: './00002-b.md'\n---\n# A\n",
    )
    .unwrap();
    std::fs::write(
        tasks.join("00002-b.md"),
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nparent: './00001-a.md'\n---\n# B\n",
    )
    .unwrap();

    let response = send(&state, "GET", "/api/projects/test/tasks/OPP-1/tree", None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let view = body_json(response).await;
    assert_eq!(view["cycles"], json!(["OPP-1"]));
}

// A task a branch agrees with its merge-base about has no matrix cell to describe it there. The
// branch still carries it, so a read scoped to that branch must say so — and must not answer with
// an empty branch set where the list route answers with one entry for the same task.
#[tokio::test]
async fn a_branch_scoped_read_always_names_the_branch_it_answered_for() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "t@example.com"]);
    git(root, &["config", "user.name", "Test"]);
    std::fs::create_dir_all(root.join(".plan/tasks")).unwrap();
    std::fs::write(root.join(".plan/config.toml"), "abbreviation = \"OPP\"\n").unwrap();
    write_alpha(root, "todo", "Alpha");
    std::fs::write(
        root.join(".plan/tasks/00002-shared.md"),
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Shared\n",
    )
    .unwrap();
    git(root, &["add", "."]);
    git_commit_at(root, 1_000_000_000, "init");
    git(root, &["checkout", "-q", "-b", "feature"]);
    write_alpha(root, "done", "Alpha done");
    git_commit_at(root, 1_000_000_100, "edit alpha only");
    git(root, &["checkout", "-q", "main"]);
    let store = op_store::Store::discover(root).unwrap();
    let repo = op_git::Repo::discover(root).unwrap();
    let state = project_state(root, repo, store);

    let detail = send(
        &state,
        "GET",
        "/api/projects/test/tasks/OPP-2?branch=feature",
        None,
    )
    .await;
    assert_eq!(detail.status(), StatusCode::OK);
    let detail = body_json(detail).await;
    assert_eq!(detail["title"], "Shared");
    let feature = detail["branches"]
        .as_array()
        .unwrap()
        .iter()
        .find(|state| state["branch"] == "feature")
        .unwrap_or_else(|| panic!("the branch it answered for is named: {detail}"));
    assert_eq!(feature["kind"], "base", "it agrees with the merge-base");

    // The list route asks a different question — this branch's task set — so it carries that one
    // branch alone. Both must agree about how the task stands on the branch they were asked about.
    let listed = send(
        &state,
        "GET",
        "/api/projects/test/tasks?branch=feature",
        None,
    )
    .await;
    let listed = body_json(listed).await;
    let shared = listed
        .as_array()
        .unwrap()
        .iter()
        .find(|task| task["id"] == "OPP-2")
        .expect("the branch lists it too")
        .clone();
    assert_eq!(shared["branches"], json!([feature]));
}

// Naming no branch has to mean the same thing on a task and on its tree. A tree built from another
// branch's task set would give the task a different parent and different children than the detail
// read beside it.
#[tokio::test]
async fn a_branchless_tree_and_a_branchless_detail_answer_for_one_branch() {
    let (dir, state) = git_state();
    // A child of alpha that only `feature` carries, so the two branches disagree about the tree.
    std::fs::write(
        dir.path().join(".plan/tasks/00002-kid.md"),
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nparent: './00001-alpha.md'\n---\n# Kid\n",
    )
    .unwrap();
    git(dir.path(), &["checkout", "-q", "feature"]);
    git(dir.path(), &["add", "."]);
    git_commit_at(dir.path(), 1_000_000_200, "feature gains a child");
    git(dir.path(), &["checkout", "-q", "main"]);

    let detail = body_json(send(&state, "GET", "/api/projects/test/tasks/OPP-1", None).await).await;
    assert_eq!(detail["headline"], "feature", "the version that headlines");

    let tree =
        body_json(send(&state, "GET", "/api/projects/test/tasks/OPP-1/tree", None).await).await;
    let children = tree["tree"]["children"].as_array().unwrap();
    assert_eq!(
        children.len(),
        1,
        "the tree is built from the branch the detail headlined: {tree}"
    );
    assert_eq!(children[0]["id"], "OPP-2");
    assert_eq!(
        tree["tree"]["title"], detail["title"],
        "and the two describe the same version"
    );
}

async fn create_tag(state: &AppState, body: Value) -> Response {
    send(state, "POST", "/api/projects/test/tags", Some(body)).await
}

async fn tag_names(state: &AppState) -> Vec<String> {
    let list = send(state, "GET", "/api/projects/test/tags", None).await;
    assert_eq!(list.status(), StatusCode::OK);
    body_json(list)
        .await
        .as_array()
        .unwrap()
        .iter()
        .map(|tag| tag["name"].as_str().unwrap().to_owned())
        .collect()
}

async fn create_task_with(state: &AppState, body: Value) -> Response {
    send(state, "POST", "/api/projects/test/tasks", Some(body)).await
}

#[tokio::test]
async fn tags_crud_roundtrip() {
    let (_dir, state) = store_state();

    let created = create_tag(
        &state,
        json!({ "name": "Front End", "color": "violet", "description": "The web client." }),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let view = body_json(created).await;
    assert_eq!(view["name"], "front-end", "the name is normalized");
    assert_eq!(view["display"], "Front End", "the heading keeps the case");
    assert_eq!(view["color"], "violet");
    assert_eq!(view["description"], "The web client.");

    assert_eq!(tag_names(&state).await, vec!["front-end".to_owned()]);

    let got = send(&state, "GET", "/api/projects/test/tags/front-end", None).await;
    assert_eq!(got.status(), StatusCode::OK);
    assert_eq!(body_json(got).await, view);

    let patched = send(
        &state,
        "PATCH",
        "/api/projects/test/tags/front-end",
        Some(json!({ "color": "teal", "description": "The SPA." })),
    )
    .await;
    assert_eq!(patched.status(), StatusCode::OK);
    let view = body_json(patched).await;
    assert_eq!(view["color"], "teal");
    assert_eq!(view["description"], "The SPA.");

    let deleted = send(&state, "DELETE", "/api/projects/test/tags/front-end", None).await;
    assert_eq!(deleted.status(), StatusCode::NO_CONTENT);
    assert!(tag_names(&state).await.is_empty());
}

#[tokio::test]
async fn a_tag_without_a_color_is_given_one() {
    let (_dir, state) = store_state();

    let created = create_tag(&state, json!({ "name": "backend" })).await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let view = body_json(created).await;
    assert!(
        view["color"].is_string(),
        "an omitted color is derived from the name, not left unset"
    );
    assert!(
        view.get("description").is_none(),
        "a tag with no prose carries no description"
    );
}

#[tokio::test]
async fn creating_a_registered_tag_again_is_a_conflict() {
    let (_dir, state) = store_state();

    assert_eq!(
        create_tag(&state, json!({ "name": "backend" }))
            .await
            .status(),
        StatusCode::CREATED
    );
    let again = create_tag(&state, json!({ "name": "Backend" })).await;
    assert_eq!(
        again.status(),
        StatusCode::CONFLICT,
        "both spellings normalize to one name"
    );
}

#[tokio::test]
async fn a_color_outside_the_palette_is_refused() {
    let (_dir, state) = store_state();

    // The palette is a closed enum, so a name outside it never reaches the store: the body fails
    // to deserialize, the same 422 every other closed field answers with.
    let created = create_tag(&state, json!({ "name": "backend", "color": "chartreuse" })).await;
    assert_eq!(created.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn a_name_that_cannot_be_normalized_is_refused() {
    let (_dir, state) = store_state();

    let created = create_tag(&state, json!({ "name": "C++" })).await;
    assert_eq!(created.status(), StatusCode::BAD_REQUEST);
    let message = body_json(created).await["message"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(
        message.contains("lowercase letters"),
        "the refusal names the rule: {message}"
    );
}

#[tokio::test]
async fn a_tag_no_branch_registers_is_not_found() {
    let (_dir, state) = store_state();

    let got = send(&state, "GET", "/api/projects/test/tags/backend", None).await;
    assert_eq!(got.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn deleting_a_referenced_tag_needs_force() {
    let (_dir, state) = store_state();
    assert_eq!(
        create_tag(&state, json!({ "name": "backend" }))
            .await
            .status(),
        StatusCode::CREATED
    );
    let created = create_task_with(
        &state,
        json!({ "title": "Wire the parser", "tags": ["backend"] }),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);

    let refused = send(&state, "DELETE", "/api/projects/test/tags/backend", None).await;
    assert_eq!(refused.status(), StatusCode::CONFLICT);
    // The status covers three refusals and `force` answers only this one, so the caller reads the
    // field rather than the sentence to decide whether to offer it.
    let body = body_json(refused).await;
    assert_eq!(body["reason"], "tag_referenced");
    assert!(
        !body["message"].as_str().unwrap().contains("--force"),
        "the remedy is the caller's own spelling, not the store's: {body}"
    );
    assert_eq!(tag_names(&state).await, vec!["backend".to_owned()]);

    let forced = send(
        &state,
        "DELETE",
        "/api/projects/test/tags/backend?force=true",
        None,
    )
    .await;
    assert_eq!(forced.status(), StatusCode::NO_CONTENT);
    assert!(tag_names(&state).await.is_empty());
}

#[tokio::test]
async fn renaming_a_tag_rewrites_the_tasks_that_reference_it() {
    let (_dir, state) = store_state();
    assert_eq!(
        create_tag(&state, json!({ "name": "backend" }))
            .await
            .status(),
        StatusCode::CREATED
    );
    let created = create_task_with(
        &state,
        json!({ "title": "Wire the parser", "tags": ["backend"] }),
    )
    .await;
    let id = body_json(created).await["id"].as_str().unwrap().to_owned();

    let renamed = send(
        &state,
        "PATCH",
        "/api/projects/test/tags/backend",
        Some(json!({ "name": "Infra" })),
    )
    .await;
    assert_eq!(renamed.status(), StatusCode::OK);
    let view = body_json(renamed).await;
    assert_eq!(view["name"], "infra");
    assert_eq!(view["display"], "Infra");
    assert_eq!(tag_names(&state).await, vec!["infra".to_owned()]);

    let task = send(
        &state,
        "GET",
        &format!("/api/projects/test/tasks/{id}"),
        None,
    )
    .await;
    assert_eq!(
        body_json(task).await["metadata"]["tags"],
        json!(["infra"]),
        "the reference moved with the tag"
    );
}

#[tokio::test]
async fn a_rename_keeps_the_color_and_the_description_it_does_not_name() {
    let (_dir, state) = store_state();
    assert_eq!(
        create_tag(
            &state,
            json!({ "name": "backend", "color": "amber", "description": "Behind the API." })
        )
        .await
        .status(),
        StatusCode::CREATED
    );

    let renamed = send(
        &state,
        "PATCH",
        "/api/projects/test/tags/backend",
        Some(json!({ "name": "Infra" })),
    )
    .await;
    assert_eq!(renamed.status(), StatusCode::OK);
    let view = body_json(renamed).await;
    assert_eq!(view["name"], "infra");
    assert_eq!(view["color"], "amber");
    assert_eq!(view["description"], "Behind the API.");
}

#[tokio::test]
async fn a_rename_that_only_changes_the_case_moves_the_heading_alone() {
    let (_dir, state) = store_state();
    assert_eq!(
        create_tag(&state, json!({ "name": "backend" }))
            .await
            .status(),
        StatusCode::CREATED
    );

    let renamed = send(
        &state,
        "PATCH",
        "/api/projects/test/tags/backend",
        Some(json!({ "name": "Backend" })),
    )
    .await;
    assert_eq!(renamed.status(), StatusCode::OK);
    let view = body_json(renamed).await;
    assert_eq!(view["name"], "backend");
    assert_eq!(view["display"], "Backend");
    assert_eq!(tag_names(&state).await, vec!["backend".to_owned()]);
}

#[tokio::test]
async fn a_patch_can_clear_a_description() {
    let (_dir, state) = store_state();
    assert_eq!(
        create_tag(
            &state,
            json!({ "name": "backend", "description": "Behind the API." })
        )
        .await
        .status(),
        StatusCode::CREATED
    );

    let patched = send(
        &state,
        "PATCH",
        "/api/projects/test/tags/backend",
        Some(json!({ "description": null })),
    )
    .await;
    assert_eq!(patched.status(), StatusCode::OK);
    let view = body_json(patched).await;
    assert!(
        view.get("description").is_none(),
        "a cleared description leaves no prose behind: {view}"
    );
    assert_eq!(view["display"], "backend", "the heading stays");
}

#[tokio::test]
async fn renaming_onto_a_registered_name_is_a_conflict() {
    let (_dir, state) = store_state();
    for name in ["backend", "infra"] {
        assert_eq!(
            create_tag(&state, json!({ "name": name })).await.status(),
            StatusCode::CREATED
        );
    }

    let renamed = send(
        &state,
        "PATCH",
        "/api/projects/test/tags/backend",
        Some(json!({ "name": "infra" })),
    )
    .await;
    assert_eq!(renamed.status(), StatusCode::CONFLICT);
    assert_eq!(
        tag_names(&state).await,
        vec!["backend".to_owned(), "infra".to_owned()],
        "a refused rename leaves both tags alone"
    );
}

#[tokio::test]
async fn a_task_can_only_carry_registered_tags() {
    let (_dir, state) = store_state();

    let refused = create_task_with(
        &state,
        json!({ "title": "Wire the parser", "tags": ["backend"] }),
    )
    .await;
    assert_eq!(refused.status(), StatusCode::BAD_REQUEST);
    let body = body_json(refused).await;
    assert_eq!(body["reason"], "tag_unregistered");
    let message = body["message"].as_str().unwrap();
    assert!(
        message.contains("backend"),
        "the refusal names the tag: {message}"
    );
    assert!(
        !message.contains("openplan"),
        "the remedy is the caller's own spelling, not the store's: {message}"
    );

    assert_eq!(
        create_tag(&state, json!({ "name": "backend" }))
            .await
            .status(),
        StatusCode::CREATED
    );
    let created = create_task_with(
        &state,
        json!({ "title": "Wire the parser", "tags": ["backend"] }),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let id = body_json(created).await["id"].as_str().unwrap().to_owned();

    let patched = send(
        &state,
        "PATCH",
        &format!("/api/projects/test/tasks/{id}"),
        Some(json!({ "tags": ["backend", "wip"] })),
    )
    .await;
    assert_eq!(
        patched.status(),
        StatusCode::BAD_REQUEST,
        "the whole set is validated, not only what the patch adds"
    );

    let cleared = send(
        &state,
        "PATCH",
        &format!("/api/projects/test/tasks/{id}"),
        Some(json!({ "tags": [] })),
    )
    .await;
    assert_eq!(cleared.status(), StatusCode::OK);
    assert_eq!(body_json(cleared).await["metadata"]["tags"], json!([]));
}

#[tokio::test]
async fn a_tag_write_is_announced_on_the_event_stream() {
    let (_dir, state) = store_state();

    // The GET resolves once the handler has subscribed, so a change published afterwards is
    // buffered for this receiver rather than lost.
    let events = send(&state, "GET", "/api/events", None).await;
    assert_eq!(events.status(), StatusCode::OK);

    assert_eq!(
        create_tag(&state, json!({ "name": "backend" }))
            .await
            .status(),
        StatusCode::CREATED
    );

    let event: Value = serde_json::from_str(&first_sse_data(events).await).unwrap();
    assert_eq!(event["kind"], "tags_changed");
    assert_eq!(event["project"], PROJECT);
    assert_eq!(event["branch"], "main");
}

async fn created_task(state: &AppState, title: &str) -> String {
    let response = send(
        state,
        "POST",
        "/api/projects/test/tasks",
        Some(json!({ "title": title })),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    body_json(response).await["id"].as_str().unwrap().to_owned()
}

#[tokio::test]
async fn comments_append_and_read_back() {
    let (_dir, state) = store_state();
    let id = created_task(&state, "Ship login").await;
    let path = format!("/api/projects/test/tasks/{id}/comments");

    let response = send(
        &state,
        "POST",
        &path,
        Some(json!({ "text": "hello", "author": "Milan Suk", "agent": "claude-code" })),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let written = body_json(response).await;
    assert_eq!(written["author"], "Milan Suk");
    assert_eq!(written["agent"], "claude-code");
    assert_eq!(written["text"], "hello");

    let read = body_json(send(&state, "GET", &path, None).await).await;
    assert_eq!(read.as_array().unwrap().len(), 1);
    assert_eq!(read[0]["text"], "hello");
    assert_eq!(read[0]["at"], written["at"]);
}

#[tokio::test]
async fn a_comment_never_reaches_the_detail_body() {
    let (_dir, state) = store_state();
    let id = created_task(&state, "Ship login").await;
    send(
        &state,
        "POST",
        &format!("/api/projects/test/tasks/{id}/comments"),
        Some(json!({ "text": "hello", "author": "Milan Suk" })),
    )
    .await;

    let detail = body_json(
        send(
            &state,
            "GET",
            &format!("/api/projects/test/tasks/{id}"),
            None,
        )
        .await,
    )
    .await;

    assert!(
        !detail["body"].as_str().unwrap().contains("Comments"),
        "the body carries no log: {}",
        detail["body"]
    );
    assert_eq!(detail["comments"][0]["text"], "hello");
    assert_eq!(detail["comments"][0]["agent"], Value::Null);
}

#[tokio::test]
async fn a_patch_keeps_the_comment_log_out_of_the_body_it_echoes() {
    let (_dir, state) = store_state();
    let id = created_task(&state, "Ship login").await;
    send(
        &state,
        "POST",
        &format!("/api/projects/test/tasks/{id}/comments"),
        Some(json!({ "text": "hello", "author": "Milan Suk" })),
    )
    .await;

    let echoed = body_json(
        send(
            &state,
            "PATCH",
            &format!("/api/projects/test/tasks/{id}"),
            Some(json!({ "status": "in_progress" })),
        )
        .await,
    )
    .await;

    assert!(!echoed["body"].as_str().unwrap().contains("Comments"));
    assert_eq!(echoed["comments"][0]["text"], "hello");
}

#[tokio::test]
async fn a_list_row_counts_the_comments() {
    let (_dir, state) = store_state();
    let id = created_task(&state, "Ship login").await;
    for text in ["one", "two"] {
        send(
            &state,
            "POST",
            &format!("/api/projects/test/tasks/{id}/comments"),
            Some(json!({ "text": text, "author": "Milan Suk" })),
        )
        .await;
    }

    let items = body_json(send(&state, "GET", "/api/projects/test/tasks", None).await).await;

    assert_eq!(items[0]["comment_count"], 2);
}

#[tokio::test]
async fn an_empty_or_unsigned_comment_is_refused() {
    let (_dir, state) = store_state();
    let id = created_task(&state, "Ship login").await;
    let path = format!("/api/projects/test/tasks/{id}/comments");

    let empty = send(
        &state,
        "POST",
        &path,
        Some(json!({ "text": "  \n ", "author": "Milan Suk" })),
    )
    .await;
    assert_eq!(empty.status(), StatusCode::BAD_REQUEST);

    let unsigned = send(
        &state,
        "POST",
        &path,
        Some(json!({ "text": "a", "author": "" })),
    )
    .await;
    assert_eq!(unsigned.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn comments_of_a_missing_task_are_404() {
    let (_dir, state) = store_state();
    let response = send(
        &state,
        "GET",
        "/api/projects/test/tasks/OPP-9/comments",
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_branch_read_groups_every_branch_that_holds_the_task() {
    let (dir, state) = git_state();
    let root = dir.path();
    let store = worktree_store(root);
    let mut task = store.read(number("OPP-1")).unwrap();
    task.append_comment(&op_task::comment::NewComment {
        at: "2026-01-01T00:00:00Z".parse().unwrap(),
        author: "Milan Suk".to_owned(),
        agent: None,
        text: "on main".to_owned(),
    });
    store.write(number("OPP-1"), &task).unwrap();

    let groups = body_json(
        send(
            &state,
            "GET",
            "/api/projects/test/tasks/OPP-1/comments/branches",
            None,
        )
        .await,
    )
    .await;

    let branches: Vec<&str> = groups
        .as_array()
        .unwrap()
        .iter()
        .map(|group| group["branch"].as_str().unwrap())
        .collect();
    assert_eq!(branches, vec!["feature", "main"]);
    let main = groups
        .as_array()
        .unwrap()
        .iter()
        .find(|group| group["branch"] == "main")
        .unwrap();
    assert_eq!(main["comments"][0]["text"], "on main");
    let feature = groups
        .as_array()
        .unwrap()
        .iter()
        .find(|group| group["branch"] == "feature")
        .unwrap();
    assert_eq!(feature["comments"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn a_search_matches_comment_text() {
    let (_dir, state) = store_state();
    let id = created_task(&state, "Ship login").await;
    send(
        &state,
        "POST",
        &format!("/api/projects/test/tasks/{id}/comments"),
        Some(json!({ "text": "the parser mishandles a tab", "author": "Milan Suk" })),
    )
    .await;

    let hits = body_json(
        send(
            &state,
            "GET",
            "/api/projects/test/search?q=mishandles&fresh=true",
            None,
        )
        .await,
    )
    .await;

    assert_eq!(hits.as_array().unwrap().len(), 1, "{hits}");
    assert_eq!(hits[0]["task"]["id"], id);
}

#[tokio::test]
async fn a_line_break_in_the_identity_is_refused() {
    let (_dir, state) = store_state();
    let id = created_task(&state, "Ship login").await;
    let path = format!("/api/projects/test/tasks/{id}/comments");

    let forged = send(
        &state,
        "POST",
        &path,
        Some(json!({
            "text": "real",
            "author": "Evil",
            "agent": "x\n\n### 2026-01-02T00:00:00Z by Forged\n\n> forged entry",
        })),
    )
    .await;
    assert_eq!(forged.status(), StatusCode::BAD_REQUEST);
    assert!(message_of(&body_json(forged).await).contains("line break"));

    let signed = send(
        &state,
        "POST",
        &path,
        Some(json!({ "text": "real", "author": "Ada\nBogus" })),
    )
    .await;
    assert_eq!(signed.status(), StatusCode::BAD_REQUEST);

    let read = body_json(send(&state, "GET", &path, None).await).await;
    assert_eq!(read.as_array().unwrap().len(), 0, "{read}");
}
