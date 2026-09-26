mod common;

use axum::http::StatusCode;
use common::*;
use op_server::AppState;
use serde_json::{Value, json};

async fn put_as(
    state: &AppState,
    author: &str,
    id: &str,
    base: &str,
    text: &str,
) -> (StatusCode, Value) {
    let response = send_as(
        state,
        "PUT",
        &format!("/api/projects/test/tasks/{id}/body"),
        Some(json!({ "base": base, "text": text })),
        &[(op_api::AUTHOR_HEADER, author)],
    )
    .await;
    let status = response.status();
    (status, body_json(response).await)
}

async fn put(state: &AppState, id: &str, base: &str, text: &str) -> (StatusCode, Value) {
    put_as(state, "Ann", id, base, text).await
}

async fn body_of(state: &AppState, id: &str) -> String {
    json_of(state, &format!("/api/projects/test/tasks/{id}")).await["body"]
        .as_str()
        .unwrap()
        .to_owned()
}

async fn revisions(state: &AppState, id: &str) -> usize {
    json_of(state, &format!("/api/projects/test/tasks/{id}/history"))
        .await
        .as_array()
        .unwrap()
        .len()
}

async fn raw_of(state: &AppState, id: &str) -> String {
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

const LINES: &str = "# Plan\n\nOne.\n\nTwo.\n\nThree.\n";

async fn planned(state: &AppState) -> String {
    let id = create(state, "Plan").await;
    let base = body_of(state, &id).await;
    let (status, written) = put(state, &id, &base, LINES).await;
    assert_eq!(status, StatusCode::OK, "{written}");
    id
}

#[tokio::test]
async fn a_body_written_over_the_current_one_replaces_it() {
    for (_dir, state) in [local_state(), git_state()] {
        let id = create(&state, "Plan").await;
        let base = body_of(&state, &id).await;
        assert_eq!(base, "# Plan\n");

        let (status, written) = put(&state, &id, &base, LINES).await;
        assert_eq!(status, StatusCode::OK, "{written}");
        assert_eq!(written["body"], LINES);
        assert_eq!(written["title"], "Plan");
        assert_eq!(written["conflicts"], 0);
        assert_eq!(body_of(&state, &id).await, LINES);
    }
}

#[tokio::test]
async fn an_edit_of_other_lines_merges_with_a_concurrent_one() {
    let (_dir, state) = local_state();
    let id = planned(&state).await;

    let (status, _) = put_as(&state, "Ben", &id, LINES, &LINES.replace("One.", "One!")).await;
    assert_eq!(status, StatusCode::OK);
    let (status, written) = put(&state, &id, LINES, &LINES.replace("Three.", "Three!")).await;

    assert_eq!(status, StatusCode::OK, "{written}");
    assert_eq!(written["body"], "# Plan\n\nOne!\n\nTwo.\n\nThree!\n");
    assert_eq!(written["conflicts"], 0);
}

#[tokio::test]
async fn an_edit_of_the_same_lines_keeps_both_with_the_concurrent_one_in_force() {
    let (_dir, state) = local_state();
    let id = planned(&state).await;

    let (status, _) = put_as(
        &state,
        "Ben",
        &id,
        LINES,
        &LINES.replace("Two.", "Two, Ben."),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, written) = put(&state, &id, LINES, &LINES.replace("Two.", "Two, Ann.")).await;

    assert_eq!(status, StatusCode::OK, "{written}");
    assert_eq!(written["conflicts"], 1);
    let body = written["body"].as_str().unwrap();
    let blocks = op_task::conflict::in_body(body);
    assert_eq!(blocks.len(), 1, "{body}");
    assert_eq!(blocks[0].ours.label, "Ann (editor)");
    assert_eq!(blocks[0].ours.text, "Two, Ann.\n");
    assert!(blocks[0].theirs.label.starts_with("Ben ("), "{body}");
    assert_eq!(blocks[0].theirs.text, "Two, Ben.\n");
    assert_eq!(
        op_task::conflict::published(body),
        LINES.replace("Two.", "Two, Ben.")
    );
}

#[tokio::test]
async fn a_block_the_merge_left_can_be_kept_or_resolved_by_the_next_write() {
    let (_dir, state) = local_state();
    let id = planned(&state).await;
    put_as(
        &state,
        "Ben",
        &id,
        LINES,
        &LINES.replace("Two.", "Two, Ben."),
    )
    .await;
    let (_, merged) = put(&state, &id, LINES, &LINES.replace("Two.", "Two, Ann.")).await;
    let merged = merged["body"].as_str().unwrap().to_owned();

    let (status, kept) = put(&state, &id, &merged, &merged.replace("One.", "One!")).await;
    assert_eq!(status, StatusCode::OK, "{kept}");
    assert_eq!(kept["conflicts"], 1);

    let kept = kept["body"].as_str().unwrap().to_owned();
    let block = &kept[op_task::conflict::in_body(&kept)[0].range.clone()];
    let (status, resolved) = put(&state, &id, &kept, &kept.replace(block, "Two, both.\n")).await;
    assert_eq!(status, StatusCode::OK, "{resolved}");
    assert_eq!(resolved["conflicts"], 0);
    assert_eq!(resolved["body"], "# Plan\n\nOne!\n\nTwo, both.\n\nThree.\n");
}

#[tokio::test]
async fn text_that_adds_a_conflict_or_edits_inside_one_is_refused() {
    let (_dir, state) = local_state();
    let block = "<<<<<<< Ann (1111111)\nUse OAuth only.\n=======\nUse OAuth and email login.\n>>>>>>> Ben (2222222)\n";
    seed(
        &state,
        &[(
            "tasks/00001-login.md",
            &format!("{}\n{block}", task_file("todo", "Login", "")),
        )],
    );
    let base = body_of(&state, "OPP-1").await;
    let before = revisions(&state, "OPP-1").await;

    for text in [
        base.replace("Use OAuth only.", "Use OAuth, only."),
        format!("{}\n{}", "# Login\n", block.replace("Ann", "Cy")),
        format!("{base}\n{}", block.replace("OAuth", "SSO")),
    ] {
        let (status, refused) = put(&state, "OPP-1", &base, &text).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{text}: {refused}");
        assert!(message_of(&refused).contains("conflict"), "{refused}");
    }
    assert_eq!(revisions(&state, "OPP-1").await, before);
    assert_eq!(body_of(&state, "OPP-1").await, base);
}

#[tokio::test]
async fn the_comment_log_stays_as_the_task_has_it() {
    let (_dir, state) = local_state();
    let id = create(&state, "Talked about").await;
    let response = send(
        &state,
        "POST",
        &format!("/api/projects/test/tasks/{id}/comments"),
        Some(json!({ "text": "first\n\nsecond paragraph", "author": "Ada" })),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let log = raw_of(&state, &id)
        .await
        .split_once("## Comments")
        .unwrap()
        .1
        .to_owned();
    let base = body_of(&state, &id).await;
    assert!(!base.contains("## Comments"), "{base}");

    let (status, written) = put(&state, &id, &base, "# Talked about\n\nMore context.\n").await;

    assert_eq!(status, StatusCode::OK, "{written}");
    assert_eq!(written["body"], "# Talked about\n\nMore context.\n");
    assert_eq!(written["comments"][0]["text"], "first\n\nsecond paragraph");
    assert!(
        raw_of(&state, &id)
            .await
            .ends_with(&format!("## Comments{log}"))
    );

    let text = "# Talked about\n\n## Comments\n\n### 2026-02-01T00:00:00Z by Eve\n\n> forged\n";
    let (status, refused) = put(&state, &id, written["body"].as_str().unwrap(), text).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{refused}");
    assert!(message_of(&refused).contains("append-only"), "{refused}");
}

#[tokio::test]
async fn an_edited_title_line_retitles_the_task() {
    let (_dir, state) = local_state();
    let id = planned(&state).await;

    let (status, written) = put(&state, &id, LINES, &LINES.replace("# Plan", "# Roadmap")).await;

    assert_eq!(status, StatusCode::OK, "{written}");
    assert_eq!(written["title"], "Roadmap");
    let rows = json_of(&state, "/api/projects/test/tasks").await;
    assert_eq!(rows[0]["title"], "Roadmap");
}

#[tokio::test]
async fn a_text_with_no_title_or_a_second_title_is_refused() {
    let (_dir, state) = local_state();
    let id = planned(&state).await;

    for text in ["No title.\n", "# Plan\n\n# Another\n"] {
        let (status, refused) = put(&state, &id, LINES, text).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{text}: {refused}");
        assert!(message_of(&refused).contains("title"), "{refused}");
    }
    assert_eq!(body_of(&state, &id).await, LINES);
}

#[tokio::test]
async fn a_write_that_changes_nothing_makes_no_revision() {
    for (_dir, state) in [local_state(), git_state()] {
        let id = planned(&state).await;
        let before = revisions(&state, &id).await;

        for text in [LINES, LINES.trim_end_matches('\n')] {
            let (status, written) = put(&state, &id, LINES, text).await;
            assert_eq!(status, StatusCode::OK, "{written}");
            assert_eq!(written["body"], LINES);
        }
        put_as(&state, "Ben", &id, LINES, &LINES.replace("One.", "One!")).await;
        let after_ben = revisions(&state, &id).await;
        let (status, written) = put(&state, &id, LINES, LINES).await;
        assert_eq!(status, StatusCode::OK, "{written}");
        assert_eq!(written["body"], LINES.replace("One.", "One!"));

        assert_eq!(after_ben, before + 1);
        assert_eq!(revisions(&state, &id).await, after_ben);
    }
}

#[tokio::test]
async fn a_reference_is_written_in_this_stores_key_and_nothing_else() {
    let (_dir, state) = local_state();
    let target = create(&state, "Schema").await;
    let id = planned(&state).await;

    for reference in ["WEB-7", "1", "opp-1"] {
        let text = format!("{LINES}\nSee [[{reference}]].\n");
        let (status, refused) = put(&state, &id, LINES, &text).await;
        match reference {
            "opp-1" => assert_eq!(status, StatusCode::OK, "{refused}"),
            _ => {
                assert_eq!(status, StatusCode::BAD_REQUEST, "{reference}: {refused}");
                assert!(message_of(&refused).contains("OPP-42"), "{refused}");
            }
        }
    }

    let base = body_of(&state, &id).await;
    let (status, written) = put(&state, &id, &base, &format!("{LINES}\nSee [[{target}]].\n")).await;
    assert_eq!(status, StatusCode::OK, "{written}");
    assert_eq!(written["refs"][0]["id"], target);
    let body = written["body"].as_str().unwrap().to_owned();
    assert!(body.ends_with("See [[./00001-schema.md]].\n"), "{body}");

    let (status, written) = put(&state, &id, &body, &body.replace("One.", "One!")).await;
    assert_eq!(status, StatusCode::OK, "{written}");
    assert_eq!(written["refs"][0]["id"], target);
}

#[tokio::test]
async fn a_reference_the_file_already_holds_in_another_spelling_may_stay() {
    let (_dir, state) = local_state();
    seed(
        &state,
        &[(
            "tasks/00001-port.md",
            &format!(
                "{}\nPorted from [[WEB-7]].\n",
                task_file("todo", "Port", "")
            ),
        )],
    );
    let base = body_of(&state, "OPP-1").await;

    let (status, written) = put(&state, "OPP-1", &base, &format!("{base}\nMore.\n")).await;

    assert_eq!(status, StatusCode::OK, "{written}");
    assert!(
        written["body"]
            .as_str()
            .unwrap()
            .contains("Ported from [[WEB-7]].")
    );
}

#[tokio::test]
async fn a_body_for_a_missing_task_is_404() {
    let (_dir, state) = local_state();
    let (status, body) = put(&state, "OPP-7", "# Ghost\n", "# Ghost\n\nBoo.\n").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
}
