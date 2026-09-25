mod common;

use axum::http::{StatusCode, header};
use common::*;
use op_backend::Actor;
use serde_json::json;

fn sequence(event: &Sse) -> (String, u64) {
    let id = event
        .id
        .as_deref()
        .expect("a published event carries an id");
    let (boot, seq) = id.rsplit_once('-').expect("an id is <boot>-<number>");
    (
        boot.to_owned(),
        seq.parse().expect("the number is an integer"),
    )
}

#[tokio::test]
async fn events_endpoint_is_an_event_stream() {
    let (_dir, state) = local_state();
    let response = send(&state, "GET", "/api/events", None).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "text/event-stream"
    );
}

// The GET resolves once the handler has subscribed, so a change published afterwards waits in the
// stream rather than being lost.
#[tokio::test]
async fn a_task_write_is_announced_on_the_event_stream() {
    let (_dir, state) = local_state();
    state.start_projects();
    let mut events = EventStream::open(&state, None).await;

    let id = create(&state, "Wire the SSE").await;
    let created = events.expect().await;
    assert_eq!(
        created.data,
        json!({ "kind": "task_changed", "project": PROJECT, "id": id })
    );

    send(
        &state,
        "POST",
        &format!("/api/projects/test/tasks/{id}/comments"),
        Some(json!({ "text": "hello", "author": "Ada" })),
    )
    .await;
    assert_eq!(events.expect().await.data["id"], id);

    send(
        &state,
        "DELETE",
        &format!("/api/projects/test/tasks/{id}"),
        None,
    )
    .await;
    assert_eq!(events.expect().await.data["id"], id);
}

#[tokio::test]
async fn a_tag_write_is_announced_on_the_event_stream() {
    let (_dir, state) = local_state();
    state.start_projects();
    let mut events = EventStream::open(&state, None).await;

    let created = send(
        &state,
        "POST",
        "/api/projects/test/tags",
        Some(json!({ "name": "backend" })),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);

    let event = events.expect().await;
    assert_eq!(
        event.data,
        json!({ "kind": "tags_changed", "project": PROJECT })
    );
}

#[tokio::test]
async fn events_stream_ends_on_shutdown_with_final_event() {
    let (_dir, state) = local_state();
    let events = EventStream::open(&state, None).await;

    let shutdown = send_as(
        &state,
        "POST",
        "/admin/shutdown",
        None,
        &[(op_api::ADMIN_HEADER, "1")],
    )
    .await;
    assert_eq!(shutdown.status(), StatusCode::OK);

    let kinds = events.kinds_to_the_end().await;
    assert_eq!(kinds.last().map(String::as_str), Some("daemon_stopping"));
}

#[tokio::test]
async fn every_event_carries_a_number_and_a_reconnect_replays_what_it_missed() {
    let (_dir, state) = local_state();
    state.start_projects();
    let mut live = EventStream::open(&state, None).await;

    create(&state, "One").await;
    let first = live.expect().await;
    create(&state, "Two").await;
    let second = live.expect().await;
    let (boot, seq) = sequence(&first);
    assert_eq!(
        sequence(&second),
        (boot, seq + 1),
        "one daemon numbers its events in order"
    );

    let mut reconnected = EventStream::open(&state, first.id.as_deref()).await;
    let replayed = reconnected.expect().await;
    assert_eq!(replayed.id, second.id);
    assert_eq!(replayed.data, second.data);

    create(&state, "Three").await;
    let third = reconnected.expect().await;
    assert_eq!(
        third.data["id"], "OPP-3",
        "the replay hands over to the live events"
    );
    assert_eq!(sequence(&third).1, seq + 2);

    let mut current = EventStream::open(&state, third.id.as_deref()).await;
    create(&state, "Four").await;
    assert_eq!(
        current.expect().await.data["id"],
        "OPP-4",
        "a cursor at the newest event replays nothing"
    );
}

// A cursor from another daemon lifetime, or one this daemon never issued, cannot say what the client
// missed, so the client reads everything on screen again.
#[tokio::test]
async fn a_cursor_the_daemon_cannot_serve_gets_a_resync() {
    let (_dir, state) = local_state();
    state.start_projects();
    for cursor in ["nonsense", "0-1", "ffffffffff-1"] {
        let mut events = EventStream::open(&state, Some(cursor)).await;
        let first = events.expect().await;
        assert_eq!(first.data, json!({ "kind": "resync" }), "{cursor}");
        assert!(first.id.is_none(), "{cursor}: a resync is no position");
    }

    let mut events = EventStream::open(&state, Some("nonsense")).await;
    assert_eq!(events.expect().await.data["kind"], "resync");
    create(&state, "After").await;
    assert_eq!(events.expect().await.data["kind"], "task_changed");
}

// Another process holds the same storage. The watchdog tick reads what it wrote, and the stream
// announces it like a write of the daemon's own.
#[tokio::test]
async fn a_write_by_another_process_is_announced_after_the_watchdog_tick() {
    for (_dir, state) in [local_state(), git_state()] {
        state.start_projects();
        let project = project(&state);
        let mut events = EventStream::open(&state, None).await;

        other_process(&project)
            .create_task(&Actor::new("Bob"), &new_task("From Bob"))
            .unwrap();
        project.poll();

        let event = events.find("task_changed").await;
        assert_eq!(event.data["id"], "OPP-1");
        until(|| async {
            ids(&json_of(&state, "/api/projects/test/tasks").await) == vec!["OPP-1"]
        })
        .await;
        assert_eq!(
            json_of(&state, "/api/projects/test/tasks/OPP-1").await["title"],
            "From Bob"
        );
    }
}

#[tokio::test]
async fn a_fresh_read_takes_a_write_by_another_process_and_announces_it() {
    let (_dir, state) = git_state();
    state.start_projects();
    let project = project(&state);
    let mut events = EventStream::open(&state, None).await;

    other_process(&project)
        .create_task(&Actor::new("Bob"), &new_task("From Bob"))
        .unwrap();
    assert!(
        json_of(&state, "/api/projects/test/tasks")
            .await
            .as_array()
            .unwrap()
            .is_empty(),
        "a plain read answers from the index the daemon holds"
    );

    let fresh = json_of(&state, "/api/projects/test/tasks?fresh=true").await;
    assert_eq!(ids(&fresh), vec!["OPP-1"]);
    assert_eq!(events.find("task_changed").await.data["id"], "OPP-1");
}

// A tag registered outside the daemon, by hand, reaches a UI only if the daemon reads the
// directory.
#[tokio::test]
async fn a_tag_written_by_hand_reaches_the_event_stream() {
    let (dir, state) = local_state();
    state.start_projects();
    let mut events = EventStream::open(&state, None).await;

    std::fs::write(
        dir.path().join(".plan/tags/backend.md"),
        "---\ncolor: blue\n---\n\n# Backend\n",
    )
    .unwrap();
    project(&state).poll();

    let event = events.find("tags_changed").await;
    assert_eq!(event.data["project"], PROJECT);
    let names = json_of(&state, "/api/projects/test/tags").await;
    assert!(
        names
            .as_array()
            .unwrap()
            .iter()
            .any(|tag| tag["name"] == "backend"),
        "{names}"
    );
}

#[tokio::test]
async fn a_task_edited_by_hand_is_announced_and_served() {
    let (dir, state) = local_state();
    state.start_projects();
    let id = create(&state, "Before").await;
    let mut events = EventStream::open(&state, None).await;

    std::fs::write(
        dir.path().join(".plan/tasks/00001-before.md"),
        task_file("done", "After", ""),
    )
    .unwrap();
    project(&state).poll();

    assert_eq!(events.find("task_changed").await.data["id"], id);
    until(|| async { json_of(&state, "/api/projects/test/tasks/OPP-1").await["title"] == "After" })
        .await;
}

#[tokio::test]
async fn a_project_change_is_announced() {
    let home = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let state = op_server::AppState::new([local_project("alpha", dir.path(), "AAA")])
        .with_registry(home.path().join(op_server::REGISTRY_FILE));
    let mut events = EventStream::open(&state, None).await;

    let renamed = send(
        &state,
        "PATCH",
        "/api/projects/alpha",
        Some(json!({ "name": "work" })),
    )
    .await;
    assert_eq!(renamed.status(), StatusCode::OK);
    assert_eq!(
        events.expect().await.data,
        json!({ "kind": "projects_changed" })
    );

    send(&state, "DELETE", "/api/projects/work", None).await;
    assert_eq!(
        events.expect().await.data,
        json!({ "kind": "projects_changed" })
    );
}
