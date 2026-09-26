mod common;

use axum::http::{StatusCode, header};
use common::*;
use op_backend::{Edit, Op};
use op_server::AppState;
use serde_json::{Value, json};

async fn newest(state: &AppState) -> Value {
    json_of(state, "/api/projects/test/history?limit=1").await[0].clone()
}

fn diff_uri(revision: &Value, query: &str) -> String {
    format!(
        "/api/projects/test/revisions/{}/diff?{query}",
        revision["revision"]["id"].as_str().unwrap()
    )
}

async fn diff_of(state: &AppState, revision: &Value, query: &str) -> Value {
    json_of(state, &diff_uri(revision, query)).await
}

#[tokio::test]
async fn a_modified_task_diffs_against_the_parent() {
    for (_dir, state) in [local_state(), git_state()] {
        create(&state, "One").await;
        let response = send(
            &state,
            "PATCH",
            "/api/projects/test/tasks/OPP-1",
            Some(json!({ "status": "done" })),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let entry = newest(&state).await;

        let response = send(
            &state,
            "GET",
            &diff_uri(&entry, "path=tasks/00001-one.md"),
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()[header::CACHE_CONTROL],
            "private, max-age=31536000, immutable"
        );
        let body = body_json(response).await;
        assert_eq!(body["kind"], "text");
        assert_eq!(body["truncated"], false);
        let diff = body["diff"].as_str().unwrap();
        assert!(
            diff.starts_with("--- a/tasks/00001-one.md\n+++ b/tasks/00001-one.md\n@@ -1,"),
            "{diff}"
        );
        assert!(
            diff.contains("\n-status: backlog\n+status: done\n"),
            "{diff}"
        );
    }
}

#[tokio::test]
async fn an_added_document_diffs_against_nothing() {
    for (_dir, state) in [local_state(), git_state()] {
        create(&state, "One").await;
        let entry = newest(&state).await;

        let body = diff_of(&state, &entry, "path=tasks/00001-one.md").await;
        let diff = body["diff"].as_str().unwrap();
        assert!(
            diff.starts_with("--- /dev/null\n+++ b/tasks/00001-one.md\n@@ -0,0 +1,"),
            "{diff}"
        );
        assert!(diff.contains("\n+# One\n"), "{diff}");
        assert!(!diff.contains("\n-"), "{diff}");
    }
}

// Another tool can move a task to a file with a new name, and the history then lists both files.
#[tokio::test]
async fn a_moved_task_diffs_its_old_file_against_its_new_one() {
    for (_dir, state) in [local_state(), git_state()] {
        let (from, path) = ("tasks/00001-one.md", "tasks/00001-uno.md");
        seed(&state, &[(from, &task_file("todo", "One", ""))]);
        let project = project(&state);
        let moved = task_file("todo", "Uno", "");
        project
            .tracker()
            .backend()
            .commit(project.machine(), &mut |_| {
                Ok(Edit::new(
                    "Move the task",
                    vec![Op::remove(from), Op::put(path, moved.as_bytes())],
                ))
            })
            .unwrap();
        let entry = newest(&state).await;

        let body = diff_of(&state, &entry, &format!("from={from}&path={path}")).await;
        let diff = body["diff"].as_str().unwrap();
        assert!(
            diff.starts_with(&format!("--- a/{from}\n+++ b/{path}\n")),
            "{diff}"
        );
        assert!(diff.contains("\n-# One\n+# Uno\n"), "{diff}");
    }
}

#[tokio::test]
async fn a_document_the_revision_leaves_alone_is_not_found() {
    for (_dir, state) in [local_state(), git_state()] {
        create(&state, "One").await;
        create(&state, "Two").await;
        let entry = newest(&state).await;

        let untouched = send(
            &state,
            "GET",
            &diff_uri(&entry, "path=tasks/00001-one.md"),
            None,
        )
        .await;
        assert_eq!(untouched.status(), StatusCode::NOT_FOUND);
        let unknown = send(
            &state,
            "GET",
            "/api/projects/test/revisions/999999/diff?path=tasks/00001-one.md",
            None,
        )
        .await;
        assert_eq!(unknown.status(), StatusCode::NOT_FOUND);
    }
}

#[tokio::test]
async fn a_long_diff_stops_at_the_cap() {
    for (_dir, state) in [local_state(), git_state()] {
        let long: String = (0..1000).map(|line| format!("line {line}\n")).collect();
        seed(&state, &[("assets/long.txt", &long)]);
        let entry = newest(&state).await;

        let body = diff_of(&state, &entry, "path=assets/long.txt").await;
        assert_eq!(body["truncated"], true);
        assert_eq!(body["diff"].as_str().unwrap().lines().count(), 400);
    }
}

#[tokio::test]
async fn a_binary_document_is_not_diffed() {
    for (_dir, state) in [local_state(), git_state()] {
        seed(&state, &[("assets/image.png", "\u{0}PNG")]);
        let entry = newest(&state).await;

        let body = diff_of(&state, &entry, "path=assets/image.png").await;
        assert_eq!(body, json!({ "kind": "binary" }));
    }
}
