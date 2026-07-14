use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::response::Response;
use http_body_util::BodyExt;
use op_server::{AppState, app};
use serde_json::{Value, json};
use tower::ServiceExt;

async fn get(uri: &str) -> axum::response::Response {
    app(AppState::default())
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap()
}

fn store_state() -> (tempfile::TempDir, AppState) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".plan/tasks")).unwrap();
    let store = op_store::Store::open(dir.path()).unwrap();
    (dir, AppState::default().with_store(store))
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
async fn matrix_endpoint_returns_empty() {
    let response = get("/api/matrix").await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], br#"{"cells":[]}"#);
}

#[tokio::test]
async fn health_reports_identity_when_set() {
    let info = op_api::DaemonInfo {
        pid: 4242,
        port: 9,
        version: "9.9.9".to_owned(),
        started_at: 5,
    };
    let response = app(AppState::default().with_health(info.clone()))
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
    let response = app(AppState::default())
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
    let response = app(AppState::default())
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
async fn patch_preserves_unknown_frontmatter_keys() {
    let (dir, state) = store_state();
    std::fs::write(
        dir.path().join(".plan/tasks/keep.md"),
        "---\nstatus: todo\nrank: 9\n---\n# Keep\n",
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
    assert!(raw.contains("rank: 9"), "rank must survive PATCH: {raw}");
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
    let server = tokio::spawn(op_server::serve(
        listener,
        AppState::default(),
        async move {
            let _ = rx.await;
        },
    ));

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
