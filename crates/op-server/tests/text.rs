mod common;

use axum::http::StatusCode;
use common::*;
use op_server::AppState;
use serde_json::{Value, json};

type Text<'a> = (&'a str, &'a str);

async fn put_as(
    state: &AppState,
    author: &str,
    id: &str,
    (base_title, base_description): Text<'_>,
    (title, description): Text<'_>,
) -> (StatusCode, Value) {
    let response = send_as(
        state,
        "PUT",
        &format!("/api/projects/test/tasks/{id}/text"),
        Some(json!({
            "base": { "title": base_title, "description": base_description },
            "text": { "title": title, "description": description },
        })),
        &[(op_api::AUTHOR_HEADER, author)],
    )
    .await;
    let status = response.status();
    (status, body_json(response).await)
}

async fn put(state: &AppState, id: &str, base: Text<'_>, text: Text<'_>) -> (StatusCode, Value) {
    put_as(state, "Ann", id, base, text).await
}

async fn read(state: &AppState, id: &str) -> (String, String) {
    let detail = json_of(state, &format!("/api/projects/test/tasks/{id}")).await;
    (text_of(&detail["title"]), text_of(&detail["description"]))
}

fn text_of(value: &Value) -> String {
    value.as_str().unwrap().to_owned()
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

const LINES: &str = "One.\n\nTwo.\n\nThree.\n";
const PLAN: Text = ("Plan", LINES);

async fn planned(state: &AppState) -> String {
    let id = create(state, "Plan").await;
    let (status, written) = put(state, &id, ("Plan", ""), PLAN).await;
    assert_eq!(status, StatusCode::OK, "{written}");
    id
}

#[tokio::test]
async fn a_text_written_over_the_current_one_replaces_it() {
    for (_dir, state) in [local_state(), git_state()] {
        let id = create(&state, "Plan").await;
        assert_eq!(read(&state, &id).await, ("Plan".to_owned(), String::new()));

        let (status, written) = put(&state, &id, ("Plan", ""), PLAN).await;
        assert_eq!(status, StatusCode::OK, "{written}");
        assert_eq!(written["title"], "Plan");
        assert_eq!(written["description"], LINES);
        assert_eq!(written["conflicts"], 0);
        assert_eq!(read(&state, &id).await.1, LINES);
        assert!(
            raw_of(&state, &id)
                .await
                .ends_with("---\n# Plan\n\nOne.\n\nTwo.\n\nThree.\n")
        );
    }
}

#[tokio::test]
async fn an_edit_of_other_lines_merges_with_a_concurrent_one() {
    let (_dir, state) = local_state();
    let id = planned(&state).await;

    let ben = LINES.replace("One.", "One!");
    let (status, _) = put_as(&state, "Ben", &id, PLAN, ("Plan", &ben)).await;
    assert_eq!(status, StatusCode::OK);
    let ann = LINES.replace("Three.", "Three!");
    let (status, written) = put(&state, &id, PLAN, ("Plan", &ann)).await;

    assert_eq!(status, StatusCode::OK, "{written}");
    assert_eq!(written["description"], "One!\n\nTwo.\n\nThree!\n");
    assert_eq!(written["conflicts"], 0);
}

#[tokio::test]
async fn an_edit_of_the_same_lines_keeps_both_with_the_concurrent_one_in_force() {
    let (_dir, state) = local_state();
    let id = planned(&state).await;

    let ben = LINES.replace("Two.", "Two, Ben.");
    let (status, _) = put_as(&state, "Ben", &id, PLAN, ("Plan", &ben)).await;
    assert_eq!(status, StatusCode::OK);
    let ann = LINES.replace("Two.", "Two, Ann.");
    let (status, written) = put(&state, &id, PLAN, ("Plan", &ann)).await;

    assert_eq!(status, StatusCode::OK, "{written}");
    assert_eq!(written["conflicts"], 1);
    let description = written["description"].as_str().unwrap();
    let blocks = op_task::conflict::in_body(description);
    assert_eq!(blocks.len(), 1, "{description}");
    assert_eq!(blocks[0].ours.label, "Ann (editor)");
    assert_eq!(blocks[0].ours.text, "Two, Ann.\n");
    assert!(blocks[0].theirs.label.starts_with("Ben ("), "{description}");
    assert_eq!(blocks[0].theirs.text, "Two, Ben.\n");
    assert_eq!(op_task::conflict::published(description), ben);
}

#[tokio::test]
async fn a_title_another_writer_changed_merges_with_a_description_edit() {
    let (_dir, state) = local_state();
    let id = planned(&state).await;

    let (status, _) = put_as(&state, "Ben", &id, PLAN, ("Roadmap", LINES)).await;
    assert_eq!(status, StatusCode::OK);
    let ann = LINES.replace("Three.", "Three!");
    let (status, written) = put(&state, &id, PLAN, ("Plan", &ann)).await;

    assert_eq!(status, StatusCode::OK, "{written}");
    assert_eq!(written["title"], "Roadmap");
    assert_eq!(written["description"], ann);
    assert_eq!(written["conflicts"], 0);
}

#[tokio::test]
async fn a_block_the_merge_left_can_be_kept_or_resolved_by_the_next_write() {
    let (_dir, state) = local_state();
    let id = planned(&state).await;
    let ben = LINES.replace("Two.", "Two, Ben.");
    put_as(&state, "Ben", &id, PLAN, ("Plan", &ben)).await;
    let ann = LINES.replace("Two.", "Two, Ann.");
    let (_, merged) = put(&state, &id, PLAN, ("Plan", &ann)).await;
    let merged = text_of(&merged["description"]);

    let edited = merged.replace("One.", "One!");
    let (status, kept) = put(&state, &id, ("Plan", &merged), ("Plan", &edited)).await;
    assert_eq!(status, StatusCode::OK, "{kept}");
    assert_eq!(kept["conflicts"], 1);

    let kept = text_of(&kept["description"]);
    let block = &kept[op_task::conflict::in_body(&kept)[0].range.clone()];
    let chosen = kept.replace(block, "Two, both.\n");
    let (status, resolved) = put(&state, &id, ("Plan", &kept), ("Plan", &chosen)).await;
    assert_eq!(status, StatusCode::OK, "{resolved}");
    assert_eq!(resolved["conflicts"], 0);
    assert_eq!(resolved["description"], "One!\n\nTwo, both.\n\nThree.\n");

    let before = revisions(&state, &id).await;
    let (status, again) = put(&state, &id, ("Plan", &kept), ("Plan", &chosen)).await;
    assert_eq!(status, StatusCode::OK, "{again}");
    assert_eq!(revisions(&state, &id).await, before);
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
    let (title, base) = read(&state, "OPP-1").await;
    assert_eq!(base, block);
    let before = revisions(&state, "OPP-1").await;

    for text in [
        base.replace("Use OAuth only.", "Use OAuth, only."),
        block.replace("Ann", "Cy"),
        format!("{base}\n{}", block.replace("OAuth", "SSO")),
    ] {
        let (status, refused) = put(&state, "OPP-1", (&title, &base), (&title, &text)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{text}: {refused}");
        assert!(message_of(&refused).contains("conflict"), "{refused}");
    }
    assert_eq!(revisions(&state, "OPP-1").await, before);
    assert_eq!(read(&state, "OPP-1").await.1, base);

    let (status, resolved) = put(
        &state,
        "OPP-1",
        (&title, &base),
        (&title, "Use OAuth, then email login.\n"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resolved}");
    assert_eq!(resolved["conflicts"], 0);
    assert_eq!(resolved["description"], "Use OAuth, then email login.\n");
}

#[tokio::test]
async fn a_block_that_names_a_task_by_an_old_file_name_matches_its_key() {
    let (_dir, state) = local_state();
    let block = "<<<<<<< Ann (1111111)\nSee [[./00001-old-name.md]].\n=======\nSkip it.\n>>>>>>> Ben (2222222)\n";
    seed(
        &state,
        &[
            ("tasks/00001-schema.md", &task_file("todo", "Schema", "")),
            (
                "tasks/00002-login.md",
                &format!("{}\n{block}", task_file("todo", "Login", "")),
            ),
        ],
    );
    let (title, base) = read(&state, "OPP-2").await;
    assert_eq!(base, block.replace("./00001-old-name.md", "OPP-1"));

    let (status, kept) = put(
        &state,
        "OPP-2",
        (&title, &base),
        (&title, &format!("{base}\nMore.\n")),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{kept}");
    assert_eq!(kept["conflicts"], 1);

    let kept = text_of(&kept["description"]);
    let chosen = kept.replace(&base, "See [[OPP-1]].\n");
    let (status, resolved) = put(&state, "OPP-2", (&title, &kept), (&title, &chosen)).await;
    assert_eq!(status, StatusCode::OK, "{resolved}");
    assert_eq!(resolved["conflicts"], 0);
    assert_eq!(resolved["description"], "See [[OPP-1]].\n\nMore.\n");
    assert!(
        raw_of(&state, "OPP-2")
            .await
            .contains("See [[./00001-schema.md]].")
    );
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
    let base = read(&state, &id).await;
    assert_eq!(base, ("Talked about".to_owned(), String::new()));

    let text = ("Talked about", "More context.\n");
    let (status, written) = put(&state, &id, ("Talked about", ""), text).await;

    assert_eq!(status, StatusCode::OK, "{written}");
    assert_eq!(written["description"], "More context.\n");
    assert_eq!(written["comments"][0]["text"], "first\n\nsecond paragraph");
    assert!(raw_of(&state, &id).await.ends_with(&format!(
        "# Talked about\n\nMore context.\n\n## Comments{log}"
    )));

    let forged = "## Comments\n\n### 2026-02-01T00:00:00Z by Eve\n\n> forged\n";
    let (status, refused) = put(&state, &id, text, ("Talked about", forged)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{refused}");
    assert!(message_of(&refused).contains("append-only"), "{refused}");
}

#[tokio::test]
async fn a_new_title_retitles_the_task() {
    let (_dir, state) = local_state();
    let id = planned(&state).await;

    let (status, written) = put(&state, &id, PLAN, ("Roadmap", LINES)).await;

    assert_eq!(status, StatusCode::OK, "{written}");
    assert_eq!(written["title"], "Roadmap");
    assert_eq!(written["description"], LINES);
    let rows = json_of(&state, "/api/projects/test/tasks").await;
    assert_eq!(rows[0]["title"], "Roadmap");
    assert!(
        raw_of(&state, &id)
            .await
            .ends_with("---\n# Roadmap\n\nOne.\n\nTwo.\n\nThree.\n")
    );
}

#[tokio::test]
async fn an_empty_title_a_title_of_two_lines_or_a_second_title_is_refused() {
    let (_dir, state) = local_state();
    let id = planned(&state).await;

    for text in [
        ("", LINES),
        ("  ", LINES),
        ("Plan\nB", LINES),
        ("Plan", "# Another\n"),
    ] {
        let (status, refused) = put(&state, &id, PLAN, text).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{text:?}: {refused}");
        assert!(message_of(&refused).contains("title"), "{refused}");
    }
    assert_eq!(
        read(&state, &id).await,
        ("Plan".to_owned(), LINES.to_owned())
    );
}

#[tokio::test]
async fn a_new_title_for_a_title_inside_a_conflict_block_is_refused() {
    let (_dir, state) = local_state();
    let body =
        "<<<<<<< Ann (1111111)\n# Log in\n=======\n# Sign in\n>>>>>>> Ben (2222222)\n\nBody.\n";
    seed(
        &state,
        &[(
            "tasks/00001-login.md",
            &format!("---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n{body}"),
        )],
    );
    let (title, description) = read(&state, "OPP-1").await;
    assert_eq!(title, "Sign in");
    assert_eq!(description, body);

    let edited = description.replace("Body.", "More.");
    let (status, refused) = put(&state, "OPP-1", (&title, &description), ("Login", &edited)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{refused}");
    assert!(message_of(&refused).contains("conflict block"), "{refused}");

    let (status, written) = put(&state, "OPP-1", (&title, &description), (&title, &edited)).await;
    assert_eq!(status, StatusCode::OK, "{written}");
    assert_eq!(written["description"], edited);
}

#[tokio::test]
async fn a_write_that_changes_nothing_makes_no_revision() {
    for (_dir, state) in [local_state(), git_state()] {
        let id = planned(&state).await;
        let before = revisions(&state, &id).await;

        for description in [LINES, LINES.trim_end_matches('\n')] {
            let (status, written) = put(&state, &id, PLAN, ("Plan", description)).await;
            assert_eq!(status, StatusCode::OK, "{written}");
            assert_eq!(written["description"], LINES);
        }
        let ben = LINES.replace("One.", "One!");
        put_as(&state, "Ben", &id, PLAN, ("Plan", &ben)).await;
        let after_ben = revisions(&state, &id).await;
        let (status, written) = put(&state, &id, PLAN, PLAN).await;
        assert_eq!(status, StatusCode::OK, "{written}");
        assert_eq!(written["description"], ben);

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
        let (status, refused) = put(&state, &id, PLAN, ("Plan", &text)).await;
        match reference {
            "opp-1" => assert_eq!(status, StatusCode::OK, "{refused}"),
            "WEB-7" => {
                assert_eq!(status, StatusCode::BAD_REQUEST, "{refused}");
                assert!(
                    message_of(&refused).contains("WEB-7 is in another project"),
                    "{refused}"
                );
            }
            _ => {
                assert_eq!(status, StatusCode::BAD_REQUEST, "{reference}: {refused}");
                assert!(message_of(&refused).contains("OPP-42"), "{refused}");
            }
        }
    }

    let (title, base) = read(&state, &id).await;
    let text = format!("{LINES}\nSee [[{target}#Design]].\n");
    let (status, written) = put(&state, &id, (&title, &base), (&title, &text)).await;
    assert_eq!(status, StatusCode::OK, "{written}");
    assert_eq!(written["refs"][0]["id"], target);
    let description = text_of(&written["description"]);
    assert!(
        description.ends_with("See [[OPP-1#Design]].\n"),
        "{description}"
    );
    assert!(!description.contains("# Plan"), "{description}");
    assert!(
        raw_of(&state, &id)
            .await
            .ends_with("See [[./00001-schema.md#Design]].\n")
    );

    let edited = description.replace("One.", "One!");
    let (status, written) = put(&state, &id, (&title, &description), (&title, &edited)).await;
    assert_eq!(status, StatusCode::OK, "{written}");
    assert_eq!(written["refs"][0]["id"], target);
    assert_eq!(written["description"], edited);
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
    let (title, base) = read(&state, "OPP-1").await;
    assert_eq!(base, "Ported from [[WEB-7]].\n");

    let text = format!("{base}\nMore.\n");
    let (status, written) = put(&state, "OPP-1", (&title, &base), (&title, &text)).await;

    assert_eq!(status, StatusCode::OK, "{written}");
    assert_eq!(written["description"], text);
}

#[tokio::test]
async fn a_text_for_a_missing_task_is_404() {
    let (_dir, state) = local_state();
    let (status, body) = put(&state, "OPP-7", ("Ghost", ""), ("Ghost", "Boo.\n")).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
}
