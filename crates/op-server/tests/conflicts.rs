mod common;

use axum::http::StatusCode;
use common::*;
use op_api::TaskDetail;
use op_server::AppState;
use serde_json::{Value, json};

const BLOCK: &str = "<<<<<<< Ann (1111111)\nUse OAuth only.\n=======\nUse OAuth and email login.\n>>>>>>> Ben (2222222)\n";

// A task as sync writes it after Ann and Ben changed its status, its parent, and one line.
fn seeded() -> (tempfile::TempDir, AppState) {
    let (dir, state) = local_state();
    seed(
        &state,
        &[
            ("tasks/00001-root.md", &task_file("todo", "Root", "")),
            (
                "tasks/00002-login.md",
                &format!(
                    "---\n<<<<<<< Ann (1111111)\nstatus: done\n=======\nstatus: cancelled\n>>>>>>> Ben (2222222)\ncreated: 2026-01-01T00:00:00Z\n<<<<<<< Ann (1111111)\nparent: ./00001-root.md\n=======\n>>>>>>> Ben (2222222)\n---\n# Login\n\n{BLOCK}"
                ),
            ),
        ],
    );
    (dir, state)
}

async fn detail(state: &AppState) -> TaskDetail {
    serde_json::from_value(json_of(state, "/api/projects/test/tasks/OPP-2").await).unwrap()
}

async fn sent(state: &AppState, method: &str, path: &str, body: Value) -> (StatusCode, Value) {
    let response = send(
        state,
        method,
        &format!("/api/projects/test/tasks/OPP-2{path}"),
        Some(body),
    )
    .await;
    let status = response.status();
    (status, body_json(response).await)
}

#[tokio::test]
async fn a_conflict_reads_as_both_versions_with_the_published_one_in_force() {
    let (_dir, state) = seeded();

    let task = json_of(&state, "/api/projects/test/tasks/OPP-2").await;

    assert_eq!(task["conflicts"], 3);
    assert_eq!(
        task["metadata"]["status"],
        json!({
            "kind": "conflict",
            "value": "cancelled",
            "sides": [
                { "label": "Ann (1111111)", "value": "done" },
                { "label": "Ben (2222222)", "value": "cancelled" },
            ],
        })
    );
    assert_eq!(task["metadata"]["parent"]["kind"], "conflict");
    assert_eq!(task["metadata"]["parent"]["value"], Value::Null);
    assert_eq!(task["metadata"]["parent"]["sides"][0]["value"], "OPP-1");
    assert!(task["body"].as_str().unwrap().contains(BLOCK));
    assert_eq!(task["title"], "Login");
    let rows = json_of(&state, "/api/projects/test/tasks").await;
    let row = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == "OPP-2")
        .unwrap();
    assert_eq!(row["conflicts"], 3);
}

#[tokio::test]
async fn the_file_as_get_prints_it_writes_back_with_every_conflict_kept() {
    let (_dir, state) = seeded();
    let before = detail(&state).await;
    let text = op_api::render_task_file(&before.metadata, &before.body, &before.comments).unwrap();

    let (status, written) = sent(&state, "PUT", "/file", json!({ "text": text })).await;

    assert_eq!(status, StatusCode::OK, "{written}\n{text}");
    assert_eq!(written["conflicts"], 3);
    assert_eq!(
        written["metadata"],
        serde_json::to_value(&before.metadata).unwrap()
    );
}

#[tokio::test]
async fn setting_a_field_settles_its_conflict() {
    let (_dir, state) = seeded();

    let (status, patched) = sent(&state, "PATCH", "", json!({ "status": "done" })).await;

    assert_eq!(status, StatusCode::OK, "{patched}");
    assert_eq!(patched["metadata"]["status"], "done");
    assert_eq!(patched["metadata"]["parent"]["kind"], "conflict");
    assert_eq!(patched["conflicts"], 2);
}

#[tokio::test]
async fn a_block_resolves_once_and_a_stale_one_is_refused() {
    let (_dir, state) = seeded();
    let resolve = json!({ "block": BLOCK, "text": "Use OAuth, then email login." });

    let (status, resolved) = sent(&state, "POST", "/resolve", resolve.clone()).await;
    assert_eq!(status, StatusCode::OK, "{resolved}");
    assert_eq!(resolved["conflicts"], 2);
    assert!(
        resolved["body"]
            .as_str()
            .unwrap()
            .ends_with("Use OAuth, then email login.\n")
    );

    let (status, stale) = sent(&state, "POST", "/resolve", resolve).await;
    assert_eq!(status, StatusCode::CONFLICT, "{stale}");
}

#[tokio::test]
async fn a_write_that_edits_inside_a_block_is_refused() {
    let (_dir, state) = seeded();
    let before = detail(&state).await;
    let text = op_api::render_task_file(&before.metadata, &before.body, &before.comments)
        .unwrap()
        .replace("Use OAuth only.", "Use SAML only.");

    let (status, refused) = sent(&state, "PUT", "/file", json!({ "text": text })).await;

    assert_eq!(status, StatusCode::BAD_REQUEST, "{refused}");
    assert_eq!(detail(&state).await.conflicts, 3);
}
