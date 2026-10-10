mod common;

use std::time::Duration;

use axum::http::StatusCode;
use common::*;
use op_api::BackendKind;
use op_backend::{BackendError, Op, Snapshot};
use op_server::{AppState, Location, Project};
use op_tracker::{STORE_VERSIONS, StoreVersion, StoreVersions};
use semver::Version;
use serde_json::{Value, json};

fn backlog_for_todo(snapshot: &dyn Snapshot) -> Result<Vec<Op>, BackendError> {
    let mut ops = Vec::new();
    for path in snapshot.list("tasks")? {
        let text = snapshot.read_text(&path)?.unwrap_or_default();
        ops.push(Op::put(
            path,
            text.replace("status: todo", "status: backlog"),
        ));
    }
    Ok(ops)
}

const FIRST: StoreVersion = StoreVersion {
    version: Version::new(0, 0, 1),
    released: Some("0.0.1"),
    migrate: None,
};

static RELEASED: StoreVersions = StoreVersions {
    known: &[
        FIRST,
        StoreVersion {
            version: Version::new(0, 0, 2),
            released: Some("0.0.9"),
            migrate: Some(backlog_for_todo),
        },
    ],
    retired: &[],
};

static UNRELEASED: StoreVersions = StoreVersions {
    known: &[
        FIRST,
        StoreVersion {
            version: Version::new(0, 0, 2),
            released: None,
            migrate: Some(backlog_for_todo),
        },
    ],
    retired: &[],
};

const NEWER: &str = "version = \"0.0.2\"
abbreviation = \"OPP\"\n";

async fn faults(state: &AppState) -> Vec<Value> {
    json_of(state, "/api/faults")
        .await
        .as_array()
        .unwrap()
        .clone()
}

// A store of version 0.0.1 with one task in `todo`, opened again by a daemon that knows `versions`.
fn reopened(dir: &std::path::Path, versions: &'static StoreVersions) -> AppState {
    let first = local_project(PROJECT, dir, "OPP");
    first
        .tracker()
        .create_task(&first.sign().unwrap(), &new_task("Ship it"))
        .unwrap();
    drop(first);
    let location = Location::find(dir, Some(BackendKind::Local)).unwrap();
    AppState::new([Project::open_in(PROJECT, location, versions).unwrap()])
}

fn config(state: &AppState) -> String {
    project(state)
        .tracker()
        .backend()
        .head()
        .unwrap()
        .read_text("config.toml")
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn a_newer_store_version_serves_no_task_and_says_which_release_reads_it() {
    let (_dir, state) = local_state();
    create(&state, "Ship it").await;
    seed(&state, &[("config.toml", NEWER)]);

    let faults = faults(&state).await;
    assert_eq!(faults.len(), 1, "{faults:?}");
    assert_eq!(faults[0]["kind"], "newer_store_version");
    assert_eq!(
        faults[0]["message"],
        "these tasks use store version 0.0.2, and this openplan reads store versions up to 0.0.1; they need a \
         newer openplan; run `openplan update`"
    );
    let tasks = send(&state, "GET", "/api/projects/test/tasks", None).await;
    assert_eq!(tasks.status(), StatusCode::CONFLICT);
    assert!(message_of(&body_json(tasks).await).contains("store version 0.0.2"));
}

#[tokio::test]
async fn a_newer_store_version_asks_the_daemon_to_update_and_reports_how_it_went() {
    let (_dir, state) = local_state();
    state.start_projects();
    let updates = state.self_update();
    updates.set_automatic(true);

    seed(&state, &[("config.toml", NEWER)]);

    tokio::time::timeout(Duration::from_secs(5), updates.wanted())
        .await
        .expect("the project asks for an update");
    let message = faults(&state).await[0]["message"].clone();
    assert!(
        message
            .as_str()
            .unwrap()
            .ends_with("the daemon is looking for an update now"),
        "{message}"
    );
    let outcome = "openplan 0.0.8 is the newest release, and only a canary build reads them; \
                   run `openplan update --canary`";
    updates.record(outcome.to_owned());
    let message = faults(&state).await[0]["message"].clone();
    assert!(
        message
            .as_str()
            .unwrap()
            .ends_with(&format!("newer openplan; {outcome}")),
        "{message}"
    );
}

#[tokio::test]
async fn a_daemon_migrates_an_older_store_by_itself_when_a_release_reads_the_new_store_version() {
    let dir = tempfile::tempdir().unwrap();
    let state = reopened(dir.path(), &RELEASED);

    assert_eq!(
        config(&state),
        "version = \"0.0.2\"
abbreviation = \"OPP\"\n"
    );
    assert!(faults(&state).await.is_empty());
    let task = json_of(&state, "/api/projects/test/tasks/OPP-1").await;
    assert_eq!(task["metadata"]["status"], "backlog");
    let history = json_of(&state, "/api/projects/test/history").await;
    assert_eq!(
        history[0]["summary"],
        json!(["Migrate the tasks from store version 0.0.1 to 0.0.2"])
    );
}

#[tokio::test]
async fn a_canary_serves_an_older_store_read_only_until_someone_migrates_it() {
    let dir = tempfile::tempdir().unwrap();
    let state = reopened(dir.path(), &UNRELEASED);

    assert!(config(&state).starts_with(
        "version = \"0.0.1\"
"
    ));
    let faults_before = faults(&state).await;
    assert_eq!(
        faults_before[0]["kind"], "older_store_version",
        "{faults_before:?}"
    );
    let task = json_of(&state, "/api/projects/test/tasks/OPP-1").await;
    assert_eq!(task["metadata"]["status"], "backlog");
    let refused = send(
        &state,
        "POST",
        "/api/projects/test/tasks",
        Some(json!({ "title": "Next" })),
    )
    .await;
    assert_eq!(refused.status(), StatusCode::CONFLICT);
    assert!(message_of(&body_json(refused).await).contains("run `openplan migrate`"));

    let migrated = send(&state, "POST", "/api/projects/test/migrate", None).await;
    assert_eq!(migrated.status(), StatusCode::OK);
    assert_eq!(
        body_json(migrated).await,
        json!({ "project": "test", "from": "0.0.1", "version": "0.0.2" })
    );
    assert!(faults(&state).await.is_empty());
    create(&state, "Next").await;
}

#[tokio::test]
async fn a_migration_of_a_store_in_the_current_store_version_changes_nothing() {
    let (_dir, state) = local_state();
    let migrated = send(&state, "POST", "/api/projects/test/migrate", None).await;
    assert_eq!(migrated.status(), StatusCode::OK);
    assert_eq!(
        body_json(migrated).await,
        json!({ "project": "test", "version": STORE_VERSIONS.current().to_string() })
    );
}
