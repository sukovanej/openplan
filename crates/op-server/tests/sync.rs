mod common;

use std::path::{Path, PathBuf};

use axum::http::StatusCode;
use common::*;
use op_server::{AppState, REGISTRY_FILE};
use serde_json::{Value, json};

// A bare repository stands in for the shared remote, and each person works in a clone of it.
struct Team {
    root: tempfile::TempDir,
}

impl Team {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        git(
            root.path(),
            &["init", "-q", "--bare", "-b", "main", "remote.git"],
        );
        Self { root }
    }

    fn remote(&self) -> PathBuf {
        self.root.path().join("remote.git")
    }

    fn clone(&self, name: &str) -> PathBuf {
        git(
            self.root.path(),
            &["clone", "-q", self.remote().to_str().unwrap(), name],
        );
        let path = self.root.path().join(name);
        identify(&path);
        path
    }

    fn remote_tip(&self) -> String {
        git_output(&self.remote(), &["rev-parse", op_backend_git::TASKS_REF])
    }
}

fn tip(clone: &Path) -> String {
    git_output(clone, &["rev-parse", op_backend_git::TASKS_REF])
}

// No sync loop runs here, so every exchange with the remote is one the test asks for.
fn member(path: &Path, abbreviation: Option<&str>) -> AppState {
    let project = open(PROJECT, path, op_api::BackendKind::Git);
    let project = match abbreviation {
        Some(abbreviation) => started(project, abbreviation),
        None => project,
    };
    AppState::new([project])
}

async fn sync(state: &AppState) -> Value {
    let response = send(state, "POST", "/api/projects/test/sync", None).await;
    let status = response.status();
    let body = body_json(response).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

#[tokio::test]
async fn a_project_with_no_remote_has_nothing_to_sync() {
    let (_local_dir, local) = local_state();
    let (_git_dir, git) = git_state();
    for (state, reason) in [(&local, "local directory"), (&git, "no remote")] {
        for method in ["GET", "POST"] {
            let response = send(state, method, "/api/projects/test/sync", None).await;
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "{method}");
            let message = message_of(&body_json(response).await);
            assert!(message.contains(reason), "{method}: {message}");
        }
    }
}

#[tokio::test]
async fn a_sync_pushes_the_tasks_and_reports_it() {
    let team = Team::new();
    let alice_path = team.clone("alice");
    let alice = member(&alice_path, Some("OPP"));

    let before = json_of(&alice, "/api/projects/test/sync").await;
    assert_eq!(before["remote"], "origin");
    assert!(before.get("last_attempt").is_none(), "{before}");
    assert!(before.get("last_success").is_none(), "{before}");
    assert_eq!(json_of(&alice, "/api/projects").await[0]["sync"], before);

    create(&alice, "Shared").await;
    let result = sync(&alice).await;
    assert!(result["sent"].as_u64().unwrap() >= 1, "{result}");
    assert_eq!(result["received"], 0);
    assert_eq!(result["merged"], false);
    assert!(result["status"].get("error").is_none(), "{result}");
    assert!(result["status"]["last_success"].is_string(), "{result}");
    assert_eq!(team.remote_tip(), tip(&alice_path));

    let after = json_of(&alice, "/api/projects/test/sync").await;
    assert_eq!(after, result["status"]);
    assert_eq!(
        (after["ahead"].as_u64(), after["behind"].as_u64()),
        (Some(0), Some(0))
    );

    let again = sync(&alice).await;
    assert_eq!(
        (again["sent"].as_u64(), again["received"].as_u64()),
        (Some(0), Some(0))
    );
}

#[tokio::test]
async fn a_sync_brings_in_the_tasks_a_teammate_pushed() {
    let team = Team::new();
    let alice = member(&team.clone("alice"), Some("OPP"));
    create(&alice, "From Alice").await;
    sync(&alice).await;

    let bob = member(&team.clone("bob"), None);
    let joined = sync(&bob).await;
    assert!(joined["received"].as_u64().unwrap() >= 1, "{joined}");
    assert_eq!(
        ids(&json_of(&bob, "/api/projects/test/tasks").await),
        vec!["OPP-1"]
    );
    assert_eq!(
        json_of(&bob, "/api/projects").await[0]["abbreviation"],
        "OPP"
    );

    create(&bob, "From Bob").await;
    sync(&bob).await;
    let received = sync(&alice).await;
    assert!(received["received"].as_u64().unwrap() >= 1, "{received}");
    assert_eq!(
        received["merged"], false,
        "alice had nothing new, so she fast-forwards"
    );
    let tasks = json_of(&alice, "/api/projects/test/tasks").await;
    assert_eq!(ids(&tasks), vec!["OPP-1", "OPP-2"]);
    assert_eq!(tasks[1]["title"], "From Bob");
}

// Two people who each create a task offline both take the next number. The merge keeps both
// tasks and gives one of them a new number.
#[tokio::test]
async fn a_sync_merges_two_offline_creates_and_keeps_both() {
    let team = Team::new();
    let alice = member(&team.clone("alice"), Some("OPP"));
    create(&alice, "Shared").await;
    sync(&alice).await;
    let bob = member(&team.clone("bob"), None);
    sync(&bob).await;

    assert_eq!(create(&alice, "From Alice").await, "OPP-2");
    assert_eq!(create(&bob, "From Bob").await, "OPP-2");
    sync(&alice).await;
    let merged = sync(&bob).await;
    assert_eq!(merged["merged"], true, "{merged}");
    assert!(merged["sent"].as_u64().unwrap() >= 1, "{merged}");
    sync(&alice).await;

    for state in [&alice, &bob] {
        let tasks = json_of(state, "/api/projects/test/tasks").await;
        let mut titles: Vec<&str> = tasks
            .as_array()
            .unwrap()
            .iter()
            .map(|task| task["title"].as_str().unwrap())
            .collect();
        titles.sort_unstable();
        assert_eq!(titles, vec!["From Alice", "From Bob", "Shared"], "{tasks}");
        assert_eq!(ids(&tasks), vec!["OPP-1", "OPP-2", "OPP-3"], "{tasks}");
    }
}

#[tokio::test]
async fn a_failed_sync_is_a_502_and_the_status_keeps_the_error() {
    let team = Team::new();
    let alice_path = team.clone("alice");
    let alice = member(&alice_path, Some("OPP"));
    create(&alice, "Shared").await;
    sync(&alice).await;
    let succeeded = json_of(&alice, "/api/projects/test/sync").await["last_success"].clone();

    git(
        &alice_path,
        &[
            "remote",
            "set-url",
            "origin",
            team.root.path().join("gone.git").to_str().unwrap(),
        ],
    );
    let failed = send(&alice, "POST", "/api/projects/test/sync", None).await;
    assert_eq!(failed.status(), StatusCode::BAD_GATEWAY);
    assert!(message_of(&body_json(failed).await).starts_with("sync:"));

    let status = json_of(&alice, "/api/projects/test/sync").await;
    assert!(status["error"].is_string(), "{status}");
    assert!(status["last_attempt"].is_string(), "{status}");
    assert_eq!(
        status["last_success"], succeeded,
        "a failure keeps the last success"
    );
    assert_eq!(
        json_of(&alice, "/api/projects").await[0]["sync"]["error"],
        status["error"]
    );
}

#[tokio::test]
async fn a_sync_is_announced_on_the_event_stream() {
    let team = Team::new();
    let alice = member(&team.clone("alice"), Some("OPP"));
    let mut events = EventStream::open(&alice, None).await;

    alice.start_projects();

    let event = events.find("sync_changed").await;
    assert_eq!(
        event.data,
        json!({ "kind": "sync_changed", "project": PROJECT })
    );
    until(|| async {
        json_of(&alice, "/api/projects/test/sync")
            .await
            .get("last_success")
            .is_some()
    })
    .await;
    assert!(
        !team.remote_tip().is_empty(),
        "the sync loop pushed the start"
    );
}

// A second person who starts the same repository joins the tasks the remote holds, rather than
// start a rival set.
#[tokio::test]
async fn starting_a_clone_joins_the_tasks_its_remote_holds() {
    let team = Team::new();
    let alice = member(&team.clone("alice"), Some("OPP"));
    create(&alice, "From Alice").await;
    sync(&alice).await;

    let bob_path = team.clone("bob");
    let home = tempfile::tempdir().unwrap();
    let bob = AppState::new([]).with_registry(home.path().join(REGISTRY_FILE));
    let registered = send(
        &bob,
        "POST",
        "/api/projects",
        Some(json!({ "path": bob_path, "abbreviation": "OPP" })),
    )
    .await;
    assert_eq!(registered.status(), StatusCode::CREATED);
    let view = body_json(registered).await;
    assert_eq!(view["backend"], "git");
    assert_eq!(view["abbreviation"], "OPP");
    let name = view["name"].as_str().unwrap().to_owned();
    assert_eq!(
        ids(&json_of(&bob, &format!("/api/projects/{name}/tasks")).await),
        vec!["OPP-1"],
        "the start took what the remote holds"
    );

    let rival = send(
        &bob,
        "POST",
        "/api/projects",
        Some(json!({ "path": bob_path, "abbreviation": "ZZZ" })),
    )
    .await;
    assert_eq!(rival.status(), StatusCode::CONFLICT);
}

// A clone fetches no tasks by itself. Registering it without an abbreviation fetches them.
#[tokio::test]
async fn registering_a_clone_serves_the_tasks_of_its_remote() {
    let team = Team::new();
    let alice = member(&team.clone("alice"), Some("OPP"));
    create(&alice, "From Alice").await;
    sync(&alice).await;

    let bob_path = team.clone("bob");
    let home = tempfile::tempdir().unwrap();
    let bob = AppState::new([]).with_registry(home.path().join(REGISTRY_FILE));
    let registered = send(
        &bob,
        "POST",
        "/api/projects",
        Some(json!({ "path": bob_path })),
    )
    .await;
    assert_eq!(registered.status(), StatusCode::CREATED);
    let name = body_json(registered).await["name"]
        .as_str()
        .unwrap()
        .to_owned();

    until(|| async {
        ids(&json_of(&bob, &format!("/api/projects/{name}/tasks")).await) == vec!["OPP-1"]
    })
    .await;
}
