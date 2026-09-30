mod common;

use axum::http::StatusCode;
use common::*;
use op_server::AppState;
use serde_json::{Value, json};

const OWN: &str = "https://github.com/acme/widgets/pull/214";
const OTHER: &str = "https://github.com/rust-lang/cargo/pull/1234";

// A git project whose remote is on GitHub. Nothing here syncs, so the remote is never reached.
fn github_state() -> (tempfile::TempDir, AppState) {
    let dir = tempfile::tempdir().unwrap();
    repository(dir.path());
    git(
        dir.path(),
        &["remote", "add", "origin", "git@github.com:acme/widgets.git"],
    );
    let state = AppState::new([git_project(PROJECT, dir.path(), "OPP")]);
    (dir, state)
}

async fn patch(state: &AppState, id: &str, body: Value) -> (StatusCode, Value) {
    let response = send(
        state,
        "PATCH",
        &format!("/api/projects/test/tasks/{id}"),
        Some(body),
    )
    .await;
    let status = response.status();
    (status, body_json(response).await)
}

async fn linked(state: &AppState, id: &str) -> Value {
    json_of(state, &format!("/api/projects/test/tasks/{id}")).await["metadata"]["pull_requests"]
        .clone()
}

#[tokio::test]
async fn the_project_names_the_repository_of_its_remote() {
    let (_dir, state) = github_state();
    let projects = json_of(&state, "/api/projects").await;
    assert_eq!(
        projects[0]["forge"],
        json!({ "kind": "github", "host": "github.com", "repo": "acme/widgets" })
    );

    let (_dir, state) = git_state();
    let projects = json_of(&state, "/api/projects").await;
    assert_eq!(projects[0].get("forge"), None);
}

#[tokio::test]
async fn a_number_links_a_pull_request_of_the_project_repository() {
    let (_dir, state) = github_state();
    let id = create_in(
        &state,
        PROJECT,
        json!({ "title": "Parser", "pull_requests": [format!("{OTHER}/files")] }),
    )
    .await;

    let (status, detail) = patch(&state, &id, json!({ "add_pull_requests": ["#214"] })).await;

    assert_eq!(status, StatusCode::OK, "{detail}");
    assert_eq!(detail["metadata"]["pull_requests"], json!([OWN, OTHER]));
    let views = json!([
        { "url": OWN, "forge": "github", "repo": "acme/widgets", "number": 214, "short": "#214" },
        {
            "url": OTHER,
            "forge": "github",
            "repo": "rust-lang/cargo",
            "number": 1234,
            "short": "rust-lang/cargo#1234"
        },
    ]);
    assert_eq!(detail["pull_requests"], views);
    let rows = json_of(&state, "/api/projects/test/tasks").await;
    assert_eq!(rows[0]["pull_requests"], views);
}

#[tokio::test]
async fn a_number_is_refused_when_the_project_has_no_forge() {
    for (_dir, state) in [local_state(), git_state()] {
        let id = create(&state, "Parser").await;

        let (status, refused) = patch(&state, &id, json!({ "add_pull_requests": ["214"] })).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{refused}");
        assert!(
            message_of(&refused).contains("no GitHub or GitLab remote"),
            "{refused}"
        );

        let (status, detail) = patch(&state, &id, json!({ "add_pull_requests": [OWN] })).await;
        assert_eq!(status, StatusCode::OK, "{detail}");
        assert_eq!(detail["pull_requests"][0]["short"], "acme/widgets#214");
    }
}

#[tokio::test]
async fn an_address_that_names_no_pull_request_is_refused() {
    let (_dir, state) = local_state();
    let id = create(&state, "Parser").await;

    for body in [
        json!({ "add_pull_requests": ["https://github.com/acme/widgets/issues/214"] }),
        json!({ "pull_requests": ["https://example.com/acme/widgets/pull/214"] }),
    ] {
        let (status, refused) = patch(&state, &id, body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{refused}");
        assert!(
            message_of(&refused).contains("not a pull request"),
            "{refused}"
        );
    }
    assert_eq!(linked(&state, &id).await, json!([]));
}

#[tokio::test]
async fn two_writers_that_add_at_the_same_time_both_keep_their_entry() {
    for (_dir, state) in [local_state(), git_state()] {
        let id = create(&state, "Parser").await;

        let (first, second) = tokio::join!(
            patch(&state, &id, json!({ "add_pull_requests": [OWN] })),
            patch(&state, &id, json!({ "add_pull_requests": [OTHER] })),
        );

        assert_eq!(first.0, StatusCode::OK, "{}", first.1);
        assert_eq!(second.0, StatusCode::OK, "{}", second.1);
        assert_eq!(linked(&state, &id).await, json!([OWN, OTHER]));
    }
}

#[tokio::test]
async fn remove_unlinks_one_and_the_set_replaces_all() {
    let (_dir, state) = github_state();
    let id = create_in(
        &state,
        PROJECT,
        json!({ "title": "Parser", "pull_requests": ["214", OTHER] }),
    )
    .await;

    let (status, detail) = patch(&state, &id, json!({ "remove_pull_requests": ["214"] })).await;
    assert_eq!(status, StatusCode::OK, "{detail}");
    assert_eq!(detail["metadata"]["pull_requests"], json!([OTHER]));

    let (_, detail) = patch(&state, &id, json!({ "pull_requests": ["#7"] })).await;
    assert_eq!(
        detail["metadata"]["pull_requests"],
        json!(["https://github.com/acme/widgets/pull/7"])
    );

    let (_, detail) = patch(&state, &id, json!({ "pull_requests": [] })).await;
    assert_eq!(detail["metadata"]["pull_requests"], json!([]));
    assert_eq!(detail.get("pull_requests"), None);
}

#[tokio::test]
async fn the_history_says_which_pull_request_a_revision_linked() {
    let (_dir, state) = github_state();
    let id = create(&state, "Parser").await;
    patch(&state, &id, json!({ "add_pull_requests": ["214", OTHER] })).await;
    patch(&state, &id, json!({ "remove_pull_requests": [OTHER] })).await;

    let entries = json_of(&state, &format!("/api/projects/test/tasks/{id}/history")).await;

    assert_eq!(
        entries[0]["summary"],
        json!(["OPP-1: unlinked rust-lang/cargo#1234"])
    );
    assert_eq!(
        entries[1]["summary"],
        json!(["OPP-1: linked #214, linked rust-lang/cargo#1234"])
    );
    let change = &entries[0]["tasks"][0]["fields"][0];
    assert_eq!(change["field"], "pull_requests");
    assert_eq!(
        change["from"]
            .as_array()
            .unwrap()
            .iter()
            .map(|view| view["short"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["#214", "rust-lang/cargo#1234"]
    );
    assert_eq!(change["to"][0]["short"], "#214");
    assert_eq!(change["to"].as_array().unwrap().len(), 1);
}

// `openplan tasks get` prints this rendering, and `openplan tasks write` sends it back.
#[tokio::test]
async fn a_file_rendered_from_the_task_and_written_back_keeps_the_pull_requests() {
    let (_dir, state) = local_state();
    let id = create_in(
        &state,
        PROJECT,
        json!({ "title": "Parser", "pull_requests": [OWN, OTHER] }),
    )
    .await;
    let detail: op_api::TaskDetail =
        serde_json::from_value(json_of(&state, &format!("/api/projects/test/tasks/{id}")).await)
            .unwrap();
    let rendered = op_api::render_task_file(
        &detail.metadata,
        &detail.title,
        &detail.description,
        &detail.comments,
    )
    .unwrap();

    let response = send(
        &state,
        "PUT",
        &format!("/api/projects/test/tasks/{id}/file"),
        Some(json!({ "text": rendered.replace("# Parser", "# Lexer") })),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(linked(&state, &id).await, json!([OWN, OTHER]));
}
