mod common;

use axum::http::StatusCode;
use common::*;
use op_server::AppState;
use serde_json::{Value, json};

async fn history(state: &AppState, query: &str) -> Vec<Value> {
    json_of(state, &format!("/api/projects/test/history{query}"))
        .await
        .as_array()
        .unwrap()
        .clone()
}

async fn task_history(state: &AppState, id: &str, query: &str) -> Vec<Value> {
    json_of(
        state,
        &format!("/api/projects/test/tasks/{id}/history{query}"),
    )
    .await
    .as_array()
    .unwrap()
    .clone()
}

fn revision_ids(entries: &[Value]) -> Vec<String> {
    entries
        .iter()
        .map(|entry| entry["revision"]["id"].as_str().unwrap().to_owned())
        .collect()
}

fn changed_tasks(entry: &Value) -> Vec<String> {
    entry["changes"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|change| change["task"].as_str().map(str::to_owned))
        .collect()
}

async fn patch(state: &AppState, id: &str, body: Value) {
    let response = send(
        state,
        "PATCH",
        &format!("/api/projects/test/tasks/{id}"),
        Some(body),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn the_history_lists_every_revision_newest_first() {
    for (_dir, state) in [local_state(), git_state()] {
        create(&state, "One").await;
        create(&state, "Two").await;
        patch(&state, "OPP-1", json!({ "status": "done" })).await;

        let entries = history(&state, "").await;
        assert_eq!(entries.len(), 4, "the start and three writes: {entries:?}");
        assert_eq!(changed_tasks(&entries[0]), vec!["OPP-1"]);
        assert_eq!(entries[0]["changes"][0]["kind"], "modified");
        assert_eq!(changed_tasks(&entries[1]), vec!["OPP-2"]);
        assert_eq!(entries[1]["changes"][0]["kind"], "added");
        assert_eq!(entries[1]["changes"][0]["path"], "tasks/00002-two.md");
        assert_eq!(changed_tasks(&entries[2]), vec!["OPP-1"]);

        let start = &entries[3];
        assert!(changed_tasks(start).is_empty(), "the start writes no task");
        let paths: Vec<&str> = start["changes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|change| change["path"].as_str().unwrap())
            .collect();
        assert!(paths.contains(&"config.toml"), "{paths:?}");
        assert!(start["revision"]["parents"].as_array().unwrap().is_empty());

        for pair in entries.windows(2) {
            assert_eq!(
                pair[0]["revision"]["parents"],
                json!([pair[1]["revision"]["id"]]),
                "each revision follows the one before it"
            );
            assert!(pair[0]["revision"]["at"].as_str() >= pair[1]["revision"]["at"].as_str());
        }
        for entry in &entries {
            assert!(!entry["revision"]["message"].as_str().unwrap().is_empty());
        }
    }
}

// The daemon reads each change from the documents, so it never depends on what the message says.
#[tokio::test]
async fn the_history_says_what_each_revision_changed() {
    for (_dir, state) in [local_state(), git_state()] {
        create(&state, "One").await;
        patch(
            &state,
            "OPP-1",
            json!({ "status": "done", "tags": ["bug", "feature"] }),
        )
        .await;

        let entries = history(&state, "").await;
        assert_eq!(
            entries[0]["summary"],
            json!(["OPP-1: status → done, tags → bug, feature"])
        );
        assert_eq!(
            entries[0]["tasks"],
            json!([{
                "task": "OPP-1",
                "kind": "modified",
                "title": "One",
                "fields": [
                    { "field": "status", "from": "backlog", "to": "done" },
                    { "field": "tags", "from": [], "to": ["bug", "feature"] },
                ],
            }])
        );
        assert_eq!(
            entries[1]["tasks"],
            json!([{ "task": "OPP-1", "kind": "added", "title": "One" }])
        );
        let start = &entries[2];
        assert_eq!(
            start["summary"],
            json!([
                "Start the OPP tasks",
                "tag bug: create",
                "tag draft: create",
                "tag feature: create"
            ])
        );
        assert_eq!(start["tags"][0], json!({ "tag": "bug", "kind": "added" }));
        assert!(
            start["changes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|change| change["tag"] == "bug"),
            "{start}"
        );
    }
}

#[tokio::test]
async fn the_history_pages_with_before_and_limit() {
    for (_dir, state) in [local_state(), git_state()] {
        for title in ["One", "Two", "Three", "Four"] {
            create(&state, title).await;
        }
        let all = revision_ids(&history(&state, "").await);
        assert_eq!(all.len(), 5);

        let first = revision_ids(&history(&state, "?limit=2").await);
        assert_eq!(first, all[..2]);
        let second = revision_ids(&history(&state, &format!("?limit=2&before={}", first[1])).await);
        assert_eq!(second, all[2..4]);
        let last = revision_ids(&history(&state, &format!("?before={}", second[1])).await);
        assert_eq!(last, all[4..]);
        assert!(
            history(&state, &format!("?before={}", all[4]))
                .await
                .is_empty(),
            "nothing is older than the start"
        );
    }
}

#[tokio::test]
async fn paging_from_a_revision_the_project_never_had_is_404() {
    let (_dir, local) = local_state();
    let (_git_dir, git) = git_state();
    for (state, revision) in [
        (&local, "nope"),
        (&git, "nope"),
        (&git, "0123456789012345678901234567890123456789"),
    ] {
        for uri in [
            format!("/api/projects/test/history?before={revision}"),
            format!("/api/projects/test/tasks/OPP-1/history?before={revision}"),
        ] {
            let response = send(state, "GET", &uri, None).await;
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "{uri}");
        }
    }
}

#[tokio::test]
async fn a_task_history_lists_only_the_revisions_of_its_task() {
    for (_dir, state) in [local_state(), git_state()] {
        create(&state, "One").await;
        create(&state, "Two").await;
        patch(&state, "OPP-1", json!({ "status": "in_progress" })).await;
        create(&state, "Three").await;
        let commented = send(
            &state,
            "POST",
            "/api/projects/test/tasks/OPP-1/comments",
            Some(json!({ "text": "hello", "author": "Ada" })),
        )
        .await;
        assert_eq!(commented.status(), StatusCode::CREATED);

        let entries = task_history(&state, "OPP-1", "").await;
        assert_eq!(entries.len(), 3, "create, patch, comment: {entries:?}");
        for entry in &entries {
            assert_eq!(changed_tasks(entry), vec!["OPP-1"]);
        }
        assert_eq!(entries[2]["changes"][0]["kind"], "added");

        let all = revision_ids(&entries);
        let newest = revision_ids(&task_history(&state, "OPP-1", "?limit=1").await);
        assert_eq!(newest, all[..1]);
        let older =
            revision_ids(&task_history(&state, "OPP-1", &format!("?before={}", newest[0])).await);
        assert_eq!(older, all[1..]);
    }
}

// A delete removes the task, not what happened to it.
#[tokio::test]
async fn a_deleted_task_keeps_its_history() {
    let (_dir, state) = local_state();
    create(&state, "Short lived").await;
    let deleted = send(&state, "DELETE", "/api/projects/test/tasks/OPP-1", None).await;
    assert_eq!(deleted.status(), StatusCode::NO_CONTENT);

    let entries = task_history(&state, "OPP-1", "").await;
    let kinds: Vec<&str> = entries
        .iter()
        .map(|entry| entry["changes"][0]["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, vec!["removed", "added"]);

    let before = entries[1]["revision"]["id"].as_str().unwrap();
    let at = json_of(
        &state,
        &format!("/api/projects/test/tasks/OPP-1/revisions/{before}"),
    )
    .await;
    assert_eq!(at["task"]["title"], "Short lived");
}

#[tokio::test]
async fn a_task_reads_as_it_stood_at_each_revision() {
    for (_dir, state) in [local_state(), git_state()] {
        create(&state, "Draft").await;
        patch(&state, "OPP-1", json!({ "status": "done" })).await;

        let entries = task_history(&state, "OPP-1", "").await;
        let ids = revision_ids(&entries);
        let at = |revision: &str| {
            let state = state.clone();
            let uri = format!("/api/projects/test/tasks/OPP-1/revisions/{revision}");
            async move { json_of(&state, &uri).await }
        };

        let created = at(&ids[1]).await;
        assert_eq!(created["id"], "OPP-1");
        assert_eq!(created["revision"], ids[1]);
        assert_eq!(created["task"]["title"], "Draft");
        assert_eq!(created["task"]["metadata"]["status"], "backlog");
        assert!(
            created["task"]["raw"]
                .as_str()
                .unwrap()
                .contains("status: backlog")
        );
        assert_eq!(at(&ids[0]).await["task"]["metadata"]["status"], "done");

        let start = revision_ids(&history(&state, "").await).pop().unwrap();
        let before_it_existed = at(&start).await;
        assert!(
            before_it_existed.get("task").is_none(),
            "the task did not exist then: {before_it_existed}"
        );
    }
}

#[tokio::test]
async fn a_task_at_a_revision_the_project_never_had_is_404_and_a_bad_key_is_400() {
    let (_dir, state) = local_state();
    create(&state, "Draft").await;

    let missing = send(
        &state,
        "GET",
        "/api/projects/test/tasks/OPP-1/revisions/nope",
        None,
    )
    .await;
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);

    let revision = revision_ids(&history(&state, "").await).remove(0);
    let bad_key = send(
        &state,
        "GET",
        &format!("/api/projects/test/tasks/nonsense/revisions/{revision}"),
        None,
    )
    .await;
    assert_eq!(bad_key.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_comment_at_a_revision_stays_out_of_the_body() {
    let (_dir, state) = local_state();
    create(&state, "Talked about").await;
    send(
        &state,
        "POST",
        "/api/projects/test/tasks/OPP-1/comments",
        Some(json!({ "text": "hello", "author": "Ada" })),
    )
    .await;

    let revision = revision_ids(&task_history(&state, "OPP-1", "").await).remove(0);
    let at = json_of(
        &state,
        &format!("/api/projects/test/tasks/OPP-1/revisions/{revision}"),
    )
    .await;
    assert_eq!(at["task"]["comments"][0]["text"], "hello");
    assert_eq!(at["task"]["title"], "Talked about");
    assert_eq!(at["task"]["description"], "");
    assert!(at["task"]["raw"].as_str().unwrap().contains("hello"));
}

// The CLI sends who ran it on every write, and the daemon signs the revision with it. Each header
// carries its text percent-encoded, so a name outside ASCII survives.
#[tokio::test]
async fn the_author_headers_reach_the_revision() {
    for (_dir, state) in [local_state(), git_state()] {
        let author = op_api::encode_header("Zoë Ada");
        let headers = [
            (op_api::AUTHOR_HEADER, author.as_str()),
            (op_api::EMAIL_HEADER, "zoe@example.com"),
            (op_api::AGENT_HEADER, "claude-code"),
        ];
        let writes: [(&str, &str, Value); 4] = [
            (
                "POST",
                "/api/projects/test/tasks",
                json!({ "title": "Signed" }),
            ),
            (
                "PATCH",
                "/api/projects/test/tasks/OPP-1",
                json!({ "status": "done" }),
            ),
            (
                "POST",
                "/api/projects/test/tasks/OPP-1/comments",
                json!({ "text": "hello", "author": "Zoë Ada" }),
            ),
            (
                "POST",
                "/api/projects/test/tags",
                json!({ "name": "backend" }),
            ),
        ];
        for (method, uri, body) in writes {
            let response = send_as(&state, method, uri, Some(body), &headers).await;
            assert!(response.status().is_success(), "{method} {uri}");

            let newest = history(&state, "?limit=1").await.remove(0);
            let revision = &newest["revision"];
            assert_eq!(revision["author"], "Zoë Ada", "{method} {uri}");
            assert_eq!(revision["email"], "zoe@example.com", "{method} {uri}");
            assert_eq!(revision["agent"], "claude-code", "{method} {uri}");
        }
    }
}

// The web UI sends no identity, so its writes are signed with the project's own: the git identity
// of a repository.
#[tokio::test]
async fn a_write_without_the_headers_is_signed_with_the_project_identity() {
    let (_dir, state) = git_state();
    create(&state, "Unsigned").await;
    let revision = &history(&state, "?limit=1").await[0]["revision"];
    assert_eq!(revision["author"], "Test");
    assert_eq!(revision["email"], "t@example.com");
    assert!(revision.get("agent").is_none());

    let (_local_dir, local) = local_state();
    create(&local, "Unsigned").await;
    let machine = project(&local).machine().clone();
    let revision = &history(&local, "?limit=1").await[0]["revision"];
    assert_eq!(revision["author"], machine.name.as_str());
}

// An agent that runs for a person the CLI cannot name still says which agent it is.
#[tokio::test]
async fn an_agent_header_alone_keeps_the_project_identity() {
    let (_dir, state) = git_state();
    let response = send_as(
        &state,
        "POST",
        "/api/projects/test/tasks",
        Some(json!({ "title": "Agent only" })),
        &[
            (op_api::AGENT_HEADER, "codex"),
            (op_api::AUTHOR_HEADER, "  "),
        ],
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);

    let revision = &history(&state, "?limit=1").await[0]["revision"];
    assert_eq!(revision["author"], "Test");
    assert_eq!(revision["agent"], "codex");
}

#[tokio::test]
async fn a_list_row_is_dated_by_the_newest_revision_of_its_task() {
    let (_dir, state) = local_state();
    create(&state, "One").await;
    create(&state, "Two").await;
    patch(&state, "OPP-1", json!({ "status": "done" })).await;

    let rows = json_of(&state, "/api/projects/test/tasks").await;
    for row in rows.as_array().unwrap() {
        let id = row["id"].as_str().unwrap();
        let newest = task_history(&state, id, "?limit=1").await;
        assert_eq!(row["updated"], newest[0]["revision"]["at"], "{id}");
    }
    let detail = json_of(&state, "/api/projects/test/tasks/OPP-1").await;
    assert_eq!(detail["updated"], rows[0]["updated"]);
}

// A restart dates every task from the log, not from the time it opened.
#[tokio::test]
async fn a_reopened_project_dates_its_tasks_from_the_log() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new([local_project(PROJECT, dir.path(), "OPP")]);
    create(&state, "One").await;
    let updated = json_of(&state, "/api/projects/test/tasks").await[0]["updated"].clone();
    drop(state);

    let reopened = AppState::new([open(PROJECT, dir.path(), op_api::BackendKind::Local)]);
    assert_eq!(
        json_of(&reopened, "/api/projects/test/tasks").await[0]["updated"],
        updated
    );
}
