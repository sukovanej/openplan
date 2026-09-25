mod common;

use axum::http::StatusCode;
use common::*;
use op_server::AppState;
use serde_json::{Value, json};

async fn todo(state: &AppState, project: &str, title: &str, dependencies: &[&str]) -> String {
    create_in(
        state,
        project,
        json!({ "title": title, "status": "todo", "dependencies": dependencies }),
    )
    .await
}

async fn flow(state: &AppState, query: &str) -> Value {
    json_of(state, &format!("/api/flow{query}")).await
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
    let state = AppState::new([
        local_project("alpha", alpha.path(), "AAA"),
        local_project("beta", beta.path(), "BBB"),
    ]);
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
async fn the_seeds_are_every_unfinished_task_until_a_status_narrows_them() {
    let (_alpha, _beta, state) = two_projects();
    todo(&state, "alpha", "alpha one", &[]).await;
    create_in(&state, "alpha", json!({ "title": "alpha two" })).await;
    let done = create_in(&state, "alpha", json!({ "title": "alpha three" })).await;
    let closed = send(
        &state,
        "PATCH",
        &format!("/api/projects/alpha/tasks/{done}"),
        Some(json!({ "status": "done" })),
    )
    .await;
    assert_eq!(closed.status(), StatusCode::OK);

    assert_eq!(
        waves(&flow(&state, "").await).len(),
        2,
        "the todo task and the backlog task seed, the done task does not"
    );
    assert_eq!(
        waves(&flow(&state, "?status=backlog").await),
        vec![("alpha".to_owned(), "AAA-2".to_owned(), 0)]
    );
    assert_eq!(
        waves(&flow(&state, "?status=done").await),
        vec![("alpha".to_owned(), "AAA-3".to_owned(), 0)],
        "a caller who asks for a finished task gets it"
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
