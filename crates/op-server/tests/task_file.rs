mod common;

use axum::http::StatusCode;
use common::*;
use op_server::AppState;
use serde_json::{Value, json};

async fn put(state: &AppState, id: &str, text: &str) -> (StatusCode, Value) {
    let response = send(
        state,
        "PUT",
        &format!("/api/projects/test/tasks/{id}/file"),
        Some(json!({ "text": text })),
    )
    .await;
    let status = response.status();
    (status, body_json(response).await)
}

// The file as `openplan get` prints it: the text of the newest revision of the task.
async fn file_of(state: &AppState, id: &str) -> String {
    let history = json_of(
        state,
        &format!("/api/projects/test/tasks/{id}/history?limit=1"),
    )
    .await;
    let revision = history[0]["revision"]["id"].as_str().unwrap().to_owned();
    json_of(
        state,
        &format!("/api/projects/test/tasks/{id}/revisions/{revision}"),
    )
    .await["task"]["raw"]
        .as_str()
        .unwrap()
        .to_owned()
}

async fn comment(state: &AppState, id: &str, text: &str) {
    let response = send(
        state,
        "POST",
        &format!("/api/projects/test/tasks/{id}/comments"),
        Some(json!({ "text": text, "author": "Ada" })),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn a_whole_file_replaces_the_task() {
    for (_dir, state) in [local_state(), git_state()] {
        let id = create(&state, "Draft").await;
        let text = format!(
            "{}\nThe parser must accept a tab.\n",
            task_file("todo", "Wire the parser", "rank: m\n")
        );

        let (status, written) = put(&state, &id, &text).await;
        assert_eq!(status, StatusCode::OK, "{written}");
        assert_eq!(written["id"], id);
        assert_eq!(written["title"], "Wire the parser");
        assert_eq!(written["metadata"]["status"], "todo");
        assert_eq!(written["metadata"]["rank"], "m");
        assert!(
            written["body"]
                .as_str()
                .unwrap()
                .contains("The parser must accept a tab.")
        );

        let read = json_of(&state, &format!("/api/projects/test/tasks/{id}")).await;
        assert_eq!(read["title"], written["title"]);
        assert_eq!(read["body"], written["body"]);
        assert_eq!(file_of(&state, &id).await, text);
    }
}

#[tokio::test]
async fn a_file_that_names_other_tasks_resolves_them() {
    let (_dir, state) = local_state();
    let parent = create(&state, "Epic").await;
    let dependency = create(&state, "Schema").await;
    let id = create(&state, "Draft").await;

    let text = format!(
        "{}\nSee [[{dependency}]].\n",
        task_file(
            "todo",
            "API",
            "parent: ./00001-epic.md\ndependencies:\n- ./00002-schema.md\n"
        )
    );
    let (status, written) = put(&state, &id, &text).await;
    assert_eq!(status, StatusCode::OK, "{written}");
    assert_eq!(written["metadata"]["parent"], parent);
    assert_eq!(written["parent_title"], "Epic");
    assert_eq!(written["depends_on"][0]["id"], dependency);
    assert_eq!(written["refs"][0]["id"], dependency);
}

// The comment log is append-only: a file may add entries after the ones the task has, and may
// change nothing before them.
#[tokio::test]
async fn a_file_must_keep_every_comment_the_task_has() {
    let (_dir, state) = local_state();
    let id = create(&state, "Talked about").await;
    comment(&state, &id, "first").await;
    let file = file_of(&state, &id).await;
    let (head, log) = file
        .split_once("## Comments")
        .expect("the file carries its log");

    let (status, refused) = put(&state, &id, head).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{refused}");
    assert!(message_of(&refused).contains("append-only"), "{refused}");

    let (status, refused) = put(&state, &id, &file.replace("first", "rewritten")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{refused}");
    assert_eq!(
        file_of(&state, &id).await,
        file,
        "a refused file changes nothing"
    );

    let edited = format!("{}\nMore context.\n\n## Comments{log}", head.trim_end());
    let (status, written) = put(&state, &id, &edited).await;
    assert_eq!(status, StatusCode::OK, "{written}");
    assert!(written["body"].as_str().unwrap().contains("More context."));
    assert_eq!(written["comments"][0]["text"], "first");

    let appended = format!(
        "{}\n\n### 2026-02-01T00:00:00Z by Ada\n\n> second\n",
        file_of(&state, &id).await.trim_end()
    );
    let (status, written) = put(&state, &id, &appended).await;
    assert_eq!(status, StatusCode::OK, "{written}");
    let texts: Vec<&str> = written["comments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|comment| comment["text"].as_str().unwrap())
        .collect();
    assert_eq!(texts, vec!["first", "second"]);
}

#[tokio::test]
async fn text_that_is_not_a_task_file_is_refused() {
    let (_dir, state) = local_state();
    let id = create(&state, "Draft").await;
    let before = file_of(&state, &id).await;

    for text in [
        "no frontmatter at all\n",
        "---\nstatus: todo\n---\n# No created\n",
        "---\nstatus: sideways\ncreated: 2026-01-01T00:00:00Z\n---\n# Bad status\n",
    ] {
        let (status, refused) = put(&state, &id, text).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{text}: {refused}");
    }
    assert_eq!(file_of(&state, &id).await, before);
}

#[tokio::test]
async fn a_file_is_held_to_the_rules_of_every_other_write() {
    let (_dir, state) = local_state();
    let id = create(&state, "Draft").await;

    let (status, refused) = put(&state, &id, &task_file("todo", "Tagged", "tags: [wip]\n")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(refused["reason"], "tag_unregistered");

    for parent in ["./00099-ghost.md", "./00001-draft.md", "OPP-1"] {
        let (status, refused) = put(
            &state,
            &id,
            &task_file("todo", "Moved", &format!("parent: {parent}\n")),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{parent}: {refused}");
    }

    assert_eq!(
        json_of(&state, &format!("/api/projects/test/tasks/{id}")).await["title"],
        "Draft"
    );
}

#[tokio::test]
async fn a_file_for_a_missing_task_is_404() {
    let (_dir, state) = local_state();
    let (status, body) = put(&state, "OPP-7", &task_file("todo", "Ghost", "")).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(
        json_of(&state, "/api/projects/test/tasks")
            .await
            .as_array()
            .unwrap()
            .is_empty()
    );
}
