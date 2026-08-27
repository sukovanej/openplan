use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::response::Response;
use http_body_util::BodyExt;
use op_server::{AppState, Project, app};
use serde_json::{Value, json};
use tower::ServiceExt;

fn repository(dir: &std::path::Path, abbreviation: &str) {
    git(dir, &["init", "-q", "-b", "main"]);
    git(dir, &["config", "user.email", "t@example.com"]);
    git(dir, &["config", "user.name", "Test"]);
    std::fs::create_dir_all(dir.join(".plan/tasks")).unwrap();
    std::fs::write(
        dir.join(".plan/config.toml"),
        format!("abbreviation = \"{abbreviation}\"\n"),
    )
    .unwrap();
    git(dir, &["commit", "-q", "--allow-empty", "-m", "init"]);
}

fn git(root: &std::path::Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .args(args)
        .current_dir(root)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?}");
}

fn open(name: &str, path: &std::path::Path) -> Project {
    Project::open(name, path.to_path_buf()).unwrap()
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

async fn create(state: &AppState, project: &str, body: Value) -> String {
    let response = send(
        state,
        "POST",
        &format!("/api/projects/{project}/tasks"),
        Some(body),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    body_json(response).await["id"].as_str().unwrap().to_owned()
}

async fn todo(state: &AppState, project: &str, title: &str, dependencies: &[&str]) -> String {
    create(
        state,
        project,
        json!({ "title": title, "status": "todo", "dependencies": dependencies }),
    )
    .await
}

async fn flow(state: &AppState, query: &str) -> Value {
    let response = send(state, "GET", &format!("/api/flow{query}"), None).await;
    assert_eq!(response.status(), StatusCode::OK);
    body_json(response).await
}

fn waves(flow: &Value) -> Vec<(String, String, u64)> {
    flow["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|node| node["kind"] == "leaf")
        .map(|node| {
            (
                node["project"].as_str().unwrap().to_owned(),
                node["id"].as_str().unwrap().to_owned(),
                node["wave"].as_u64().unwrap(),
            )
        })
        .collect()
}

fn two_projects() -> (tempfile::TempDir, tempfile::TempDir, AppState) {
    let alpha = tempfile::tempdir().unwrap();
    let beta = tempfile::tempdir().unwrap();
    repository(alpha.path(), "AAA");
    repository(beta.path(), "BBB");
    let state = AppState::new([open("alpha", alpha.path()), open("beta", beta.path())]);
    (alpha, beta, state)
}

#[tokio::test]
async fn the_waves_are_global_across_the_projects() {
    let (_alpha, _beta, state) = two_projects();
    let first = todo(&state, "alpha", "alpha one", &[]).await;
    todo(&state, "alpha", "alpha two", &[&first]).await;
    todo(&state, "beta", "beta one", &[]).await;

    assert_eq!(
        waves(&flow(&state, "").await),
        vec![
            ("alpha".to_owned(), "AAA-1".to_owned(), 0),
            ("beta".to_owned(), "BBB-1".to_owned(), 0),
            ("alpha".to_owned(), "AAA-2".to_owned(), 1),
        ]
    );
}

#[tokio::test]
async fn a_project_parameter_leaves_the_other_project_out() {
    let (_alpha, _beta, state) = two_projects();
    todo(&state, "alpha", "alpha one", &[]).await;
    todo(&state, "beta", "beta one", &[]).await;

    assert_eq!(
        waves(&flow(&state, "?project=beta").await),
        vec![("beta".to_owned(), "BBB-1".to_owned(), 0)]
    );
}

#[tokio::test]
async fn the_seeds_are_the_todo_tasks_until_a_status_says_otherwise() {
    let (_alpha, _beta, state) = two_projects();
    todo(&state, "alpha", "alpha one", &[]).await;
    create(&state, "alpha", json!({ "title": "alpha two" })).await;

    assert_eq!(
        waves(&flow(&state, "").await).len(),
        1,
        "the backlog task is no seed"
    );
    assert_eq!(
        waves(&flow(&state, "?status=backlog").await),
        vec![("alpha".to_owned(), "AAA-2".to_owned(), 0)]
    );
    assert_eq!(
        waves(&flow(&state, "?status=todo&status=backlog").await).len(),
        2,
        "two values of one name are alternatives"
    );
}

#[tokio::test]
async fn a_task_parameter_needs_a_project() {
    let (_alpha, _beta, state) = two_projects();
    let response = send(&state, "GET", "/api/flow?task=AAA-1", None).await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(
        body_json(response).await["message"]
            .as_str()
            .unwrap()
            .contains("needs a project")
    );
}

#[tokio::test]
async fn an_unknown_parameter_is_refused() {
    let (_alpha, _beta, state) = two_projects();
    let response = send(&state, "GET", "/api/flow?porject=alpha", None).await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        body_json(response).await["message"],
        "unknown query parameter: porject"
    );
}

#[tokio::test]
async fn an_unknown_status_is_refused() {
    let (_alpha, _beta, state) = two_projects();

    assert_eq!(
        send(&state, "GET", "/api/flow?status=review", None)
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn an_unknown_project_is_not_found() {
    let (_alpha, _beta, state) = two_projects();

    assert_eq!(
        send(&state, "GET", "/api/flow?project=gamma", None)
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn a_named_task_grows_the_flow_from_itself_alone() {
    let (_alpha, _beta, state) = two_projects();
    let first = todo(&state, "alpha", "alpha one", &[]).await;
    let second = todo(&state, "alpha", "alpha two", &[&first]).await;
    todo(&state, "alpha", "alpha three", &[&second]).await;

    assert_eq!(
        waves(&flow(&state, "?project=alpha&task=AAA-2").await),
        vec![
            ("alpha".to_owned(), "AAA-1".to_owned(), 0),
            ("alpha".to_owned(), "AAA-2".to_owned(), 1),
        ],
        "the flow takes what the task waits for, and not what waits for it"
    );
}

#[tokio::test]
async fn a_cycle_is_unprocessable_and_names_its_members() {
    let (_alpha, _beta, state) = two_projects();
    let first = todo(&state, "alpha", "alpha one", &[]).await;
    let second = todo(&state, "alpha", "alpha two", &[&first]).await;
    let patched = send(
        &state,
        "PATCH",
        &format!("/api/projects/alpha/tasks/{first}"),
        Some(json!({ "dependencies": [second] })),
    )
    .await;
    assert_eq!(patched.status(), StatusCode::OK);

    let response = send(&state, "GET", "/api/flow", None).await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body = body_json(response).await;
    assert_eq!(body["cycles"], json!([["AAA-1", "AAA-2"]]));
    assert_eq!(
        body["message"],
        "dependencies form a cycle: AAA-1 -> AAA-2 -> AAA-1"
    );
}

#[tokio::test]
async fn another_refusal_sends_no_cycles_field() {
    let (_alpha, _beta, state) = two_projects();
    let body = body_json(send(&state, "GET", "/api/flow?project=gamma", None).await).await;

    assert!(body.get("cycles").is_none());
}

#[tokio::test]
async fn a_repeated_project_sends_each_task_once() {
    let (_alpha, _beta, state) = two_projects();
    let first = todo(&state, "alpha", "alpha one", &[]).await;
    todo(&state, "alpha", "alpha two", &[&first]).await;

    assert_eq!(
        waves(&flow(&state, "?project=alpha&project=alpha").await),
        vec![
            ("alpha".to_owned(), "AAA-1".to_owned(), 0),
            ("alpha".to_owned(), "AAA-2".to_owned(), 1),
        ]
    );
}
