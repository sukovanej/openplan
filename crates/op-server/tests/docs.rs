mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::{Value, json};

#[tokio::test]
async fn a_created_doc_is_listed_and_readable_by_its_name() {
    let (_dir, state) = git_state();
    let created = send(
        &state,
        "POST",
        "/api/projects/test/docs",
        Some(json!({ "name": "System Architecture", "body": "The store is a directory." })),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let doc = body_json(created).await;
    assert_eq!(doc["name"], "system-architecture");
    assert_eq!(doc["title"], "System Architecture");
    assert_eq!(doc["body"], "The store is a directory.\n");

    let listed = body_json(send(&state, "GET", "/api/projects/test/docs", None).await).await;
    let items = listed.as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["name"], "system-architecture");

    let read = send(
        &state,
        "GET",
        "/api/projects/test/docs/system-architecture",
        None,
    )
    .await;
    assert_eq!(read.status(), StatusCode::OK);
    assert_eq!(body_json(read).await["title"], "System Architecture");
}

#[tokio::test]
async fn a_name_the_normalizer_cannot_spell_is_refused() {
    let (_dir, state) = git_state();
    let response = send(
        &state,
        "POST",
        "/api/projects/test/docs",
        Some(json!({ "name": "C++" })),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_repeated_name_is_refused_rather_than_overwriting() {
    let (_dir, state) = git_state();
    let body = json!({ "name": "Architecture" });
    send(
        &state,
        "POST",
        "/api/projects/test/docs",
        Some(body.clone()),
    )
    .await;
    let again = send(&state, "POST", "/api/projects/test/docs", Some(body)).await;
    assert_eq!(again.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn a_patch_replaces_the_body_and_a_rename_moves_the_doc() {
    let (_dir, state) = git_state();
    send(
        &state,
        "POST",
        "/api/projects/test/docs",
        Some(json!({ "name": "Architecture", "body": "old" })),
    )
    .await;

    let edited = send(
        &state,
        "PATCH",
        "/api/projects/test/docs/architecture",
        Some(json!({ "body": "new prose" })),
    )
    .await;
    assert_eq!(edited.status(), StatusCode::OK);
    assert_eq!(body_json(edited).await["body"], "new prose\n");

    let renamed = send(
        &state,
        "PATCH",
        "/api/projects/test/docs/architecture",
        Some(json!({ "name": "Storage Layout" })),
    )
    .await;
    assert_eq!(renamed.status(), StatusCode::OK);
    let doc = body_json(renamed).await;
    assert_eq!(doc["name"], "storage-layout");
    assert_eq!(doc["title"], "Storage Layout");
    assert_eq!(doc["body"], "new prose\n");

    let gone = send(&state, "GET", "/api/projects/test/docs/architecture", None).await;
    assert_eq!(gone.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_deleted_doc_leaves_the_list_empty() {
    let (_dir, state) = git_state();
    send(
        &state,
        "POST",
        "/api/projects/test/docs",
        Some(json!({ "name": "Architecture" })),
    )
    .await;
    let deleted = send(
        &state,
        "DELETE",
        "/api/projects/test/docs/architecture",
        None,
    )
    .await;
    assert_eq!(deleted.status(), StatusCode::NO_CONTENT);
    let listed = body_json(send(&state, "GET", "/api/projects/test/docs", None).await).await;
    assert!(listed.as_array().unwrap().is_empty());
}

// A doc body names tasks the way a task body does, and a task body names docs the same way back.
#[tokio::test]
async fn docs_and_tasks_resolve_each_other_s_references() {
    let (_dir, state) = git_state();
    send(
        &state,
        "POST",
        "/api/projects/test/tasks",
        Some(json!({ "title": "Ship the parser", "body": "see [[architecture]]" })),
    )
    .await;
    send(
        &state,
        "POST",
        "/api/projects/test/docs",
        Some(json!({ "name": "Architecture", "body": "built by [[OPP-1]]" })),
    )
    .await;

    let doc =
        body_json(send(&state, "GET", "/api/projects/test/docs/architecture", None).await).await;
    let refs = doc["refs"].as_array().unwrap();
    assert_eq!(
        refs.len(),
        1,
        "the doc resolves the task it names: {refs:?}"
    );
    assert_eq!(refs[0]["id"], "OPP-1");
    assert_eq!(refs[0]["title"], "Ship the parser");

    let task = body_json(send(&state, "GET", "/api/projects/test/tasks/OPP-1", None).await).await;
    let doc_refs = task["doc_refs"].as_array().unwrap();
    assert_eq!(
        doc_refs.len(),
        1,
        "the task resolves the doc it names: {doc_refs:?}"
    );
    assert_eq!(doc_refs[0]["name"], "architecture");
    assert_eq!(doc_refs[0]["title"], "Architecture");
}

// A file names every other file by its path relative to itself, and the API reads each path back
// as the key or the name a person typed.
#[tokio::test]
async fn a_reference_is_stored_as_a_relative_path_and_read_as_a_key_or_a_name() {
    let (_dir, state) = git_state();
    create(&state, "Ship the parser").await;
    send(
        &state,
        "POST",
        "/api/projects/test/docs",
        Some(
            json!({ "name": "Architecture", "body": "built by [[OPP-1]], see [[storage#Layout]]" }),
        ),
    )
    .await;
    send(
        &state,
        "PUT",
        "/api/projects/test/tasks/OPP-1/text",
        Some(json!({
            "base": { "title": "Ship the parser", "description": "" },
            "text": { "title": "Ship the parser", "description": "see [[architecture]]" },
        })),
    )
    .await;

    let plan = project(&state).tracker().plan().unwrap();
    assert!(
        plan.raw_doc("architecture").unwrap().contains(
            "built by [[../tasks/00001-ship-the-parser.md]], see [[./storage.md#Layout]]"
        )
    );
    assert!(
        plan.raw(1)
            .unwrap()
            .contains("see [[../docs/architecture.md]]")
    );
    let doc = json_of(&state, "/api/projects/test/docs/architecture").await;
    assert_eq!(doc["body"], "built by [[OPP-1]], see [[storage#Layout]]\n");
    let task = json_of(&state, "/api/projects/test/tasks/OPP-1").await;
    assert_eq!(task["description"], "see [[architecture]]\n");
}

#[tokio::test]
async fn a_missing_doc_is_a_not_found() {
    let (_dir, state) = git_state();
    let response = send(&state, "GET", "/api/projects/test/docs/nothing", None).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// The docs page reads across projects, so the merged route answers for every servable one and
// narrows to the projects the query names.
#[tokio::test]
async fn the_merged_route_spans_projects_and_narrows_by_name() {
    let (_dir, state) = git_state();
    send(
        &state,
        "POST",
        "/api/projects/test/docs",
        Some(json!({ "name": "Architecture" })),
    )
    .await;

    let all = body_json(send(&state, "GET", "/api/docs", None).await).await;
    let items = all.as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["project"], "test");
    assert_eq!(items[0]["name"], "architecture");

    let named = body_json(send(&state, "GET", "/api/docs?project=test", None).await).await;
    assert_eq!(named, all);

    let missing = send(&state, "GET", "/api/docs?project=nothing", None).await;
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);

    let bad = send(&state, "GET", "/api/docs?colour=red", None).await;
    assert_eq!(bad.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_doc_nests_under_another_one_and_both_ends_read_the_pair() {
    let (_dir, state) = git_state();
    send(
        &state,
        "POST",
        "/api/projects/test/docs",
        Some(json!({ "name": "Architecture" })),
    )
    .await;
    let created = send(
        &state,
        "POST",
        "/api/projects/test/docs",
        Some(json!({ "name": "Storage", "parent": "architecture" })),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let child = body_json(created).await;
    assert_eq!(child["metadata"]["parent"], "architecture");
    assert_eq!(child["parent_title"], "Architecture");

    let parent =
        body_json(send(&state, "GET", "/api/projects/test/docs/architecture", None).await).await;
    let children = parent["children"].as_array().unwrap();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0]["name"], "storage");
    assert_eq!(children[0]["title"], "Storage");

    let listed = body_json(send(&state, "GET", "/api/docs", None).await).await;
    let rows = listed.as_array().unwrap();
    let row = rows.iter().find(|row| row["name"] == "storage").unwrap();
    assert_eq!(row["metadata"]["parent"], "architecture");
}

#[tokio::test]
async fn a_patch_nests_a_doc_and_a_null_lifts_it_back_to_the_top_level() {
    let (_dir, state) = git_state();
    for name in ["Architecture", "Storage"] {
        send(
            &state,
            "POST",
            "/api/projects/test/docs",
            Some(json!({ "name": name })),
        )
        .await;
    }
    let nested = send(
        &state,
        "PATCH",
        "/api/projects/test/docs/storage",
        Some(json!({ "parent": "architecture" })),
    )
    .await;
    assert_eq!(nested.status(), StatusCode::OK);
    assert_eq!(
        body_json(nested).await["metadata"]["parent"],
        "architecture"
    );

    let lifted = send(
        &state,
        "PATCH",
        "/api/projects/test/docs/storage",
        Some(json!({ "parent": null })),
    )
    .await;
    assert_eq!(lifted.status(), StatusCode::OK);
    assert_eq!(body_json(lifted).await["metadata"]["parent"], Value::Null);
}

#[tokio::test]
async fn a_parent_that_closes_a_cycle_is_refused() {
    let (_dir, state) = git_state();
    for name in ["Architecture", "Storage"] {
        send(
            &state,
            "POST",
            "/api/projects/test/docs",
            Some(json!({ "name": name })),
        )
        .await;
    }
    send(
        &state,
        "PATCH",
        "/api/projects/test/docs/storage",
        Some(json!({ "parent": "architecture" })),
    )
    .await;
    let response = send(
        &state,
        "PATCH",
        "/api/projects/test/docs/architecture",
        Some(json!({ "parent": "storage" })),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_rename_moves_the_children_with_the_name() {
    let (_dir, state) = git_state();
    for name in ["Architecture", "Storage"] {
        send(
            &state,
            "POST",
            "/api/projects/test/docs",
            Some(json!({ "name": name })),
        )
        .await;
    }
    send(
        &state,
        "PATCH",
        "/api/projects/test/docs/storage",
        Some(json!({ "parent": "architecture" })),
    )
    .await;
    send(
        &state,
        "PATCH",
        "/api/projects/test/docs/architecture",
        Some(json!({ "name": "The Design" })),
    )
    .await;
    let child = body_json(send(&state, "GET", "/api/projects/test/docs/storage", None).await).await;
    assert_eq!(child["metadata"]["parent"], "the-design");
    assert_eq!(child["parent_title"], "The Design");
}

#[tokio::test]
async fn a_doc_is_dated_by_its_last_revision() {
    let (_dir, state) = local_state();
    seed(
        &state,
        &[(
            "docs/architecture.md",
            "---\ncreated: 2026-01-01T00:00:00Z\n---\n# Architecture\n",
        )],
    );
    let doc = json_of(&state, "/api/projects/test/docs/architecture").await;
    assert_eq!(doc["metadata"]["created"], "2026-01-01T00:00:00Z");
    assert_ne!(doc["updated"], "2026-01-01T00:00:00Z", "{doc}");
    assert!(doc["updated"].is_string(), "{doc}");
}

#[tokio::test]
async fn a_doc_write_is_announced_on_the_event_stream() {
    let (_dir, state) = local_state();
    state.start_projects();
    let mut events = EventStream::open(&state, None).await;
    send(
        &state,
        "POST",
        "/api/projects/test/docs",
        Some(json!({ "name": "Architecture" })),
    )
    .await;
    assert_eq!(
        events.find("doc_changed").await.data,
        json!({ "kind": "doc_changed", "project": PROJECT, "name": "architecture" })
    );
}

#[tokio::test]
async fn a_doc_another_process_wrote_is_read_on_a_fresh_read() {
    let (_dir, state) = local_state();
    let other = other_process(&project(&state));
    let doc =
        op_task::doc::Doc::new("Architecture", "2026-01-01T00:00:00Z".parse().unwrap()).unwrap();
    other
        .create_doc(&op_backend::Actor::new("Bob"), &doc)
        .unwrap();
    let listed = json_of(&state, "/api/projects/test/docs?fresh=true").await;
    assert_eq!(listed[0]["name"], "architecture");
}

async fn created(state: &op_server::AppState, body: Value) {
    let response = send(state, "POST", "/api/projects/test/docs", Some(body)).await;
    assert_eq!(response.status(), StatusCode::CREATED);
}

async fn history_count(state: &op_server::AppState) -> usize {
    json_of(state, "/api/projects/test/history")
        .await
        .as_array()
        .unwrap()
        .len()
}

#[tokio::test]
async fn a_patch_whose_rename_is_refused_writes_nothing() {
    let (_dir, state) = git_state();
    created(
        &state,
        json!({ "name": "Architecture", "body": "The old text." }),
    )
    .await;
    created(&state, json!({ "name": "Storage" })).await;
    let before = history_count(&state).await;

    for (name, status) in [
        ("Storage", StatusCode::CONFLICT),
        ("C++", StatusCode::BAD_REQUEST),
    ] {
        let refused = send(
            &state,
            "PATCH",
            "/api/projects/test/docs/architecture",
            Some(json!({ "name": name, "body": "The new text." })),
        )
        .await;
        assert_eq!(refused.status(), status, "{name}");
    }

    let doc = json_of(&state, "/api/projects/test/docs/architecture").await;
    assert_eq!(doc["body"], "The old text.\n");
    assert_eq!(history_count(&state).await, before);
}

#[tokio::test]
async fn a_patch_with_a_body_and_a_rename_is_one_revision() {
    let (_dir, state) = git_state();
    created(
        &state,
        json!({ "name": "Architecture", "body": "The old text." }),
    )
    .await;
    created(
        &state,
        json!({ "name": "Storage", "parent": "architecture" }),
    )
    .await;
    let before = history_count(&state).await;

    let patched = send(
        &state,
        "PATCH",
        "/api/projects/test/docs/architecture",
        Some(json!({ "name": "System design", "body": "The new text." })),
    )
    .await;

    assert_eq!(patched.status(), StatusCode::OK);
    let doc = body_json(patched).await;
    assert_eq!(doc["name"], "system-design");
    assert_eq!(doc["body"], "The new text.\n");
    let child = json_of(&state, "/api/projects/test/docs/storage").await;
    assert_eq!(child["metadata"]["parent"], "system-design");
    assert_eq!(history_count(&state).await, before + 1);
}

#[tokio::test]
async fn a_doc_reads_its_history_and_its_text_at_a_revision() {
    let (_dir, state) = git_state();
    created(
        &state,
        json!({ "name": "Storage", "body": "Keep it in git." }),
    )
    .await;
    send(
        &state,
        "PATCH",
        "/api/projects/test/docs/storage",
        Some(json!({ "body": "Keep it in SQLite." })),
    )
    .await;

    let history = json_of(&state, "/api/projects/test/docs/storage/history").await;
    let entries = history.as_array().unwrap();
    assert_eq!(entries.len(), 2, "{history}");
    let first = entries[1]["revision"]["id"].as_str().unwrap();
    let at = json_of(
        &state,
        &format!("/api/projects/test/docs/storage/revisions/{first}"),
    )
    .await;
    assert_eq!(at["doc"]["title"], "Storage");
    assert_eq!(at["doc"]["body"], "Keep it in git.\n");
}

#[tokio::test]
async fn a_doc_names_its_author_and_keeps_them_across_a_rename() {
    let (_dir, state) = git_state();
    created(&state, json!({ "name": "Storage" })).await;
    let doc = json_of(&state, "/api/projects/test/docs/storage").await;
    assert!(doc["author"]["name"].is_string(), "{doc}");
    let author = doc["author"].clone();

    send(
        &state,
        "PATCH",
        "/api/projects/test/docs/storage",
        Some(json!({ "name": "Storage Layout" })),
    )
    .await;

    let renamed = json_of(&state, "/api/projects/test/docs/storage-layout").await;
    assert_eq!(renamed["author"], author);
}

#[tokio::test]
async fn a_doc_reports_a_broken_diagram_a_dangling_reference_and_a_name_that_is_not_a_path() {
    let (_dir, state) = local_state();
    seed(
        &state,
        &[(
            "docs/storage.md",
            "---\ncreated: 2026-01-01T00:00:00Z\n---\n# Storage\n\nSee [[../docs/gone.md]] and [[storage]].\n\n```mermaid\ngraph TD\n  A -->\n```\n",
        )],
    );

    let doc = json_of(&state, "/api/projects/test/docs/storage").await;
    let codes: Vec<&str> = doc["problems"]
        .as_array()
        .unwrap()
        .iter()
        .map(|problem| problem["code"].as_str().unwrap())
        .collect();
    assert_eq!(codes, ["diagram", "reference", "reference_path"], "{doc}");
}

#[tokio::test]
async fn a_doc_history_runs_back_past_a_rename() {
    let (_dir, state) = git_state();
    created(&state, json!({ "name": "Architecture", "body": "First." })).await;
    send(
        &state,
        "PATCH",
        "/api/projects/test/docs/architecture",
        Some(json!({ "name": "System Architecture" })),
    )
    .await;

    let history = json_of(
        &state,
        "/api/projects/test/docs/system-architecture/history",
    )
    .await;
    let entries = history.as_array().unwrap();
    assert_eq!(entries.len(), 2, "{history}");
    assert_eq!(entries[0]["docs"][0]["doc"], "system-architecture");
    assert_eq!(entries[0]["docs"][0]["renamed_from"], "architecture");
    assert_eq!(entries[1]["docs"][0]["doc"], "architecture");
    let first = entries[1]["revision"]["id"].as_str().unwrap();
    let at = json_of(
        &state,
        &format!("/api/projects/test/docs/system-architecture/revisions/{first}"),
    )
    .await;
    assert_eq!(at["doc"]["title"], "Architecture");
}
