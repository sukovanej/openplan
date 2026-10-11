mod common;

use std::path::Path;
use std::time::Duration;

use axum::http::StatusCode;
use common::*;
use op_api::BackendKind;
use op_backend::{BackendError, Op, Snapshot};
use op_server::{AppState, Location, Project};
use op_tracker::{Retired, STORE_VERSIONS, StoreVersion, StoreVersions};
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

const TWO: StoreVersion = StoreVersion {
    number: 2,
    released: Some("0.0.1"),
    migrate: None,
};

const THREE: StoreVersion = StoreVersion {
    number: 3,
    released: Some("0.0.9"),
    migrate: Some(backlog_for_todo),
};

static RELEASED: StoreVersions = StoreVersions {
    known: &[TWO, THREE],
    retired: &[],
};

static UNRELEASED: StoreVersions = StoreVersions {
    known: &[
        TWO,
        StoreVersion {
            released: None,
            ..THREE
        },
    ],
    retired: &[],
};

static RETIRING: StoreVersions = StoreVersions {
    known: &[StoreVersion {
        migrate: None,
        ..THREE
    }],
    retired: &[Retired {
        number: 2,
        last_release: "0.0.9",
    }],
};

const NEWER: &str = "abbreviation = \"OPP\"\nversion = 3\n";

async fn faults(state: &AppState) -> Vec<Value> {
    json_of(state, "/api/faults")
        .await
        .as_array()
        .unwrap()
        .clone()
}

// A store of version 2 with one task in `todo`, opened again by a daemon that knows `versions`.
fn reopened(dir: &Path, versions: &'static StoreVersions) -> AppState {
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

async fn refused(state: &AppState, method: &str, uri: &str, body: Option<Value>) -> String {
    let response = send(state, method, uri, body).await;
    assert_eq!(response.status(), StatusCode::CONFLICT, "{method} {uri}");
    message_of(&body_json(response).await)
}

#[tokio::test]
async fn a_newer_store_version_serves_no_task_and_says_what_to_do() {
    let (_dir, state) = local_state();
    create(&state, "Ship it").await;
    seed(&state, &[("config.toml", NEWER)]);

    let faults = faults(&state).await;
    assert_eq!(faults.len(), 1, "{faults:?}");
    assert_eq!(faults[0]["kind"], "newer_store_version");
    assert_eq!(
        faults[0]["message"],
        "these tasks use store version 3, and this openplan reads store versions up to 2. They \
         need a newer openplan. Run `openplan update`"
    );
    let message = refused(&state, "GET", "/api/projects/test/tasks", None).await;
    assert!(message.contains("store version 3"), "{message}");
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
            .ends_with("They need a newer openplan. The daemon is looking for an update now"),
        "{message}"
    );
    let outcome = "openplan 0.0.8 is the newest release, and only a canary build reads them. Run \
                   `openplan update --canary`";
    updates.record(outcome.to_owned());
    let message = faults(&state).await[0]["message"].clone();
    assert!(message.as_str().unwrap().ends_with(outcome), "{message}");
}

#[tokio::test]
async fn a_retired_store_version_serves_no_task_and_hides_the_project() {
    let dir = tempfile::tempdir().unwrap();
    let state = reopened(dir.path(), &RETIRING);

    let faults = faults(&state).await;
    assert_eq!(faults[0]["kind"], "retired_store_version", "{faults:?}");
    let message = refused(&state, "GET", "/api/projects/test/tasks", None).await;
    assert!(message.contains("Run openplan 0.0.9"), "{message}");
}

#[tokio::test]
async fn a_store_from_before_the_store_versions_migrates_at_the_first_write_only() {
    let (_dir, state) = local_state();
    seed(&state, &[("config.toml", "abbreviation = \"OPP\"\n")]);

    assert_eq!(
        config(&state),
        "abbreviation = \"OPP\"\n",
        "a read writes nothing"
    );
    assert!(faults(&state).await.is_empty());
    create(&state, "Ship it").await;

    assert_eq!(config(&state), "abbreviation = \"OPP\"\nversion = 2\n");
    let history = json_of(&state, "/api/projects/test/history").await;
    let migration = history
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry.get("store_version").is_some())
        .expect("a migration in the history");
    assert_eq!(
        migration["summary"],
        json!(["Migrate the tasks from store version 1 to 2"])
    );
    assert_eq!(migration["store_version"], json!({ "from": 1, "to": 2 }));
}

#[tokio::test]
async fn a_daemon_migrates_an_older_store_at_a_write_when_a_release_reads_the_new_version() {
    let dir = tempfile::tempdir().unwrap();
    let state = reopened(dir.path(), &RELEASED);

    assert_eq!(config(&state), "abbreviation = \"OPP\"\nversion = 2\n");
    assert!(faults(&state).await.is_empty());
    let task = json_of(&state, "/api/projects/test/tasks/OPP-1").await;
    assert_eq!(task["metadata"]["status"], "backlog");

    create(&state, "Next").await;

    assert_eq!(config(&state), "abbreviation = \"OPP\"\nversion = 3\n");
}

#[tokio::test]
async fn a_canary_serves_an_older_store_read_only_until_someone_migrates_it() {
    let dir = tempfile::tempdir().unwrap();
    let state = reopened(dir.path(), &UNRELEASED);

    let faults_before = faults(&state).await;
    assert_eq!(
        faults_before[0]["kind"], "older_store_version",
        "{faults_before:?}"
    );
    let task = json_of(&state, "/api/projects/test/tasks/OPP-1").await;
    assert_eq!(task["metadata"]["status"], "backlog");
    let message = refused(
        &state,
        "POST",
        "/api/projects/test/tasks",
        Some(json!({ "title": "Next" })),
    )
    .await;
    assert!(message.contains("Run `openplan migrate`"), "{message}");

    let migrated = send(&state, "POST", "/api/projects/test/migrate", None).await;
    assert_eq!(migrated.status(), StatusCode::OK);
    assert_eq!(
        body_json(migrated).await,
        json!({ "project": "test", "from": 2, "version": 3 })
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
        json!({ "project": "test", "version": STORE_VERSIONS.current() })
    );
}

fn clone(root: &Path, name: &str) -> std::path::PathBuf {
    let remote = root.join("remote.git");
    git(root, &["clone", "-q", remote.to_str().unwrap(), name]);
    let path = root.join(name);
    identify(&path);
    path
}

// A sync that meets a newer store version asks for an update once, and the fault goes when a sync
// fails for another reason.
#[tokio::test]
async fn a_sync_that_meets_a_newer_store_version_asks_for_an_update_once() {
    let root = tempfile::tempdir().unwrap();
    git(
        root.path(),
        &["init", "-q", "--bare", "-b", "main", "remote.git"],
    );
    let alice = AppState::new([started(
        open(PROJECT, &clone(root.path(), "alice"), BackendKind::Git),
        "OPP",
    )]);
    create(&alice, "Shared").await;
    send(&alice, "POST", "/api/projects/test/sync", None).await;
    let bob_path = clone(root.path(), "bob");
    let bob = AppState::new([Project::open_in(
        PROJECT,
        Location::find(&bob_path, Some(BackendKind::Git)).unwrap(),
        &RELEASED,
    )
    .unwrap()]);
    send(&bob, "POST", "/api/projects/test/sync", None).await;
    create(&bob, "From Bob").await;
    send(&bob, "POST", "/api/projects/test/sync", None).await;
    create(&alice, "From Alice").await;
    alice.start_projects();
    let updates = alice.self_update();

    send(&alice, "POST", "/api/projects/test/sync", None).await;

    tokio::time::timeout(Duration::from_secs(5), updates.wanted())
        .await
        .expect("the sync asks for an update");
    until(|| async {
        let faults = faults(&alice).await;
        faults.len() == 1 && faults[0]["kind"] == "newer_store_version"
    })
    .await;
    send(&alice, "POST", "/api/projects/test/sync", None).await;
    assert!(
        tokio::time::timeout(Duration::from_millis(300), updates.wanted())
            .await
            .is_err(),
        "a sync that meets the same store version asks no more"
    );

    std::fs::rename(root.path().join("remote.git"), root.path().join("gone.git")).unwrap();
    send(&alice, "POST", "/api/projects/test/sync", None).await;
    until(|| async {
        let faults = faults(&alice).await;
        faults.len() == 1 && faults[0]["kind"] == "sync_failed"
    })
    .await;
}
