mod common;

use axum::http::StatusCode;
use common::*;
use op_server::AppState;
use serde_json::{Value, json};

const BLOCK: &str = "<<<<<<< Ann (1111111)\nKeep the index in memory.\n=======\nKeep the index in SQLite.\n>>>>>>> Ben (2222222)\n";

// A doc as sync writes it after Ann and Ben nested it under different docs and changed one line.
fn seeded() -> (tempfile::TempDir, AppState) {
    let (dir, state) = local_state();
    seed(
        &state,
        &[
            ("docs/guides.md", &doc_file("Guides", "")),
            ("docs/reference.md", &doc_file("Reference", "")),
            (
                "docs/storage.md",
                &format!(
                    "---\ncreated: 2026-01-01T00:00:00Z\n<<<<<<< Ann (1111111)\nparent: ./guides.md\n=======\nparent: ./reference.md\n>>>>>>> Ben (2222222)\n---\n# Storage\n\n{BLOCK}"
                ),
            ),
        ],
    );
    (dir, state)
}

fn doc_file(title: &str, frontmatter: &str) -> String {
    format!("---\ncreated: 2026-01-01T00:00:00Z\n{frontmatter}---\n# {title}\n")
}

async fn doc(state: &AppState, name: &str) -> Value {
    json_of(state, &format!("/api/projects/test/docs/{name}")).await
}

async fn sent(state: &AppState, method: &str, path: &str, body: Value) -> (StatusCode, Value) {
    let response = send(
        state,
        method,
        &format!("/api/projects/test/docs/storage{path}"),
        Some(body),
    )
    .await;
    let status = response.status();
    (status, body_json(response).await)
}

#[tokio::test]
async fn a_doc_conflict_reads_as_both_versions_with_the_published_one_in_force() {
    let (_dir, state) = seeded();

    let storage = doc(&state, "storage").await;

    assert_eq!(storage["conflicts"], 2);
    assert_eq!(
        storage["metadata"]["parent"],
        json!({
            "kind": "conflict",
            "value": "reference",
            "sides": [
                { "label": "Ann (1111111)", "value": "guides" },
                { "label": "Ben (2222222)", "value": "reference" },
            ],
        })
    );
    assert_eq!(storage["title"], "Storage");
    assert_eq!(storage["body"], BLOCK);
    assert_eq!(storage["parent_title"], "Reference");
    let rows = json_of(&state, "/api/projects/test/docs").await;
    let row = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["name"] == "storage")
        .unwrap();
    assert_eq!(row["conflicts"], 2);
}

#[tokio::test]
async fn setting_the_parent_settles_its_conflict() {
    let (_dir, state) = seeded();

    let (status, patched) = sent(&state, "PATCH", "", json!({ "parent": "guides" })).await;

    assert_eq!(status, StatusCode::OK, "{patched}");
    assert_eq!(patched["metadata"]["parent"], "guides");
    assert_eq!(patched["conflicts"], 1);
}

#[tokio::test]
async fn a_doc_text_resolves_a_block_by_replacing_it_and_refuses_an_edit_inside_one() {
    let (_dir, state) = seeded();
    let base = json!({ "title": "Storage", "body": BLOCK });
    let text = |body: &str| json!({ "base": base, "text": { "title": "Storage", "body": body } });

    let (status, refused) = sent(
        &state,
        "PUT",
        "/text",
        text(&BLOCK.replace("in memory", "on disk")),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{refused}");

    let (status, resolved) = sent(
        &state,
        "PUT",
        "/text",
        text("Keep the index in memory, and a copy in SQLite.\n"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resolved}");
    assert_eq!(resolved["conflicts"], 1);
    assert_eq!(
        resolved["body"],
        "Keep the index in memory, and a copy in SQLite.\n"
    );
}

#[tokio::test]
async fn an_edit_outside_the_blocks_keeps_them_and_an_edit_inside_one_is_refused() {
    let (_dir, state) = seeded();

    let (status, kept) = sent(
        &state,
        "PATCH",
        "",
        json!({ "body": format!("The index is small.\n\n{BLOCK}") }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{kept}");
    assert_eq!(kept["conflicts"], 2);

    let edited = BLOCK.replace("in memory", "on disk");
    let (status, refused) = sent(&state, "PATCH", "", json!({ "body": edited })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{refused}");
    assert_eq!(doc(&state, "storage").await["conflicts"], 2);
}

#[tokio::test]
async fn a_renamed_parent_carries_both_versions_of_a_child_s_conflict() {
    let (_dir, state) = seeded();

    let renamed = send(
        &state,
        "PATCH",
        "/api/projects/test/docs/guides",
        Some(json!({ "name": "Handbook" })),
    )
    .await;
    assert_eq!(renamed.status(), StatusCode::OK);

    let storage = doc(&state, "storage").await;
    assert_eq!(storage["metadata"]["parent"]["value"], "reference");
    assert_eq!(
        storage["metadata"]["parent"]["sides"][0]["value"],
        "handbook"
    );
    assert_eq!(storage["conflicts"], 2);
}
