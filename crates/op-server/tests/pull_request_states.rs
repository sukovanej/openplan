mod common;

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use common::*;
use op_api::ChangeEvent;
use op_server::{AppState, PullRequestPolling};
use serde_json::{Value, json};

const OWN: &str = "https://github.com/acme/widgets/pull/214";
const OTHER: &str = "https://github.com/rust-lang/cargo/pull/1234";
const VIEWER: &str = "ann";

// A `gh` that answers each search with `search.json`, each other query with `states.json`, and
// writes every query it gets to `calls`.
struct FakeGh {
    dir: tempfile::TempDir,
}

impl FakeGh {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let script = format!(
            "#!/bin/sh\nfor arg in \"$@\"; do\n  case \"$arg\" in\n    query=*)\n      \
             printf '%s\\n' \"$arg\" >> '{dir}/calls'\n      \
             case \"$arg\" in *'search('*) cat '{dir}/search.json' ;; *) cat '{dir}/states.json' ;; esac\n      \
             exit 0 ;;\n  esac\ndone\nexit 1\n",
            dir = dir.path().display()
        );
        let path = dir.path().join("gh");
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        let fake = Self { dir };
        fake.search(&[]);
        fake.states(json!({}));
        fake
    }

    fn program(&self) -> PathBuf {
        self.dir.path().join("gh")
    }

    fn search(&self, nodes: &[Value]) {
        let answer = json!({ "data": {
            "viewer": { "login": VIEWER },
            "search": { "pageInfo": { "hasNextPage": false, "endCursor": null }, "nodes": nodes }
        }});
        std::fs::write(self.dir.path().join("search.json"), answer.to_string()).unwrap();
    }

    fn states(&self, data: Value) {
        let answer = json!({ "data": data });
        std::fs::write(self.dir.path().join("states.json"), answer.to_string()).unwrap();
    }

    fn calls(&self) -> Vec<String> {
        std::fs::read_to_string(self.dir.path().join("calls"))
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }
}

fn node(number: u64, title: &str, head: &str, state: &str, author: &str, at: &str) -> Value {
    json!({
        "number": number, "title": title, "state": state, "isDraft": false, "headRefName": head,
        "createdAt": at, "updatedAt": at, "author": { "login": author }
    })
}

fn later(minutes: i64) -> String {
    (jiff::Timestamp::now() + jiff::SignedDuration::from_mins(minutes))
        .round(jiff::Unit::Second)
        .unwrap()
        .to_string()
}

fn github_state(polling: PullRequestPolling) -> (tempfile::TempDir, AppState) {
    let dir = tempfile::tempdir().unwrap();
    repository(dir.path());
    git(
        dir.path(),
        &["remote", "add", "origin", "git@github.com:acme/widgets.git"],
    );
    let state =
        AppState::new([git_project(PROJECT, dir.path(), "OPP")]).with_pull_request_polling(polling);
    (dir, state)
}

fn polling(gh: &Path) -> PullRequestPolling {
    PullRequestPolling {
        gh: gh.into(),
        ..PullRequestPolling::default()
    }
}

async fn linked(state: &AppState, id: &str) -> Value {
    json_of(state, &format!("/api/projects/test/tasks/{id}")).await["pull_requests"].clone()
}

async fn unlink(state: &AppState, id: &str, address: &str) {
    let response = send(
        state,
        "PATCH",
        &format!("/api/projects/test/tasks/{id}"),
        Some(json!({ "remove_pull_requests": [address] })),
    )
    .await;
    assert!(response.status().is_success());
}

#[tokio::test]
async fn a_poll_reads_the_state_of_each_linked_pull_request_and_publishes_a_change() {
    let gh = FakeGh::new();
    let (_dir, state) = github_state(polling(&gh.program()));
    let id = create_in(
        &state,
        PROJECT,
        json!({ "title": "Parser", "pull_requests": [OWN, OTHER] }),
    )
    .await;
    gh.states(json!({
        "r0": { "p214": node(214, "Fix the parser", "opp-1", "MERGED", "ben", "2026-01-01T00:00:00Z") },
        "r1": { "p1234": node(1234, "Cargo change", "x", "OPEN", "ben", "2026-01-01T00:00:00Z") },
    }));
    let mut events = state.subscribe();

    state.poll_pull_requests();

    let views = linked(&state, &id).await;
    assert_eq!(
        views[0]["status"],
        json!({ "state": "merged", "title": "Fix the parser", "updated_at": "2026-01-01T00:00:00Z" })
    );
    assert_eq!(views[1]["status"]["state"], "open");
    let event = events.recv().await.unwrap();
    assert_eq!(
        event,
        ChangeEvent::TaskChanged {
            project: PROJECT.to_owned(),
            id: id.clone()
        }
    );
    assert_eq!(gh.calls().len(), 2, "one search and one state query");

    gh.states(json!({ "r0": { "p1234": node(1234, "Cargo change", "x", "MERGED", "ben", "2026-01-02T00:00:00Z") } }));
    state.poll_pull_requests();
    let calls = gh.calls();
    let last = calls.last().unwrap();
    assert!(
        last.contains("rust-lang") && !last.contains("p214"),
        "a merged pull request of the project is not read again, and an open one elsewhere is: {last}"
    );
    assert_eq!(linked(&state, &id).await[1]["status"]["state"], "merged");
}

#[tokio::test]
async fn the_first_scan_links_no_old_pull_request() {
    let gh = FakeGh::new();
    let (_dir, state) = github_state(polling(&gh.program()));
    let id = create_in(&state, PROJECT, json!({ "title": "Parser" })).await;
    gh.search(&[node(
        300,
        &format!("Fix the parser ({id})"),
        "x",
        "OPEN",
        VIEWER,
        "2026-01-01T00:00:00Z",
    )]);

    state.poll_pull_requests();

    assert_eq!(linked(&state, &id).await, Value::Null);
}

#[tokio::test]
async fn a_new_pull_request_links_once_and_not_again_after_a_remove() {
    let gh = FakeGh::new();
    let (_dir, state) = github_state(polling(&gh.program()));
    let id = create_in(&state, PROJECT, json!({ "title": "Parser" })).await;
    state.poll_pull_requests();
    let branch = format!("{}-fix-parser", id.to_lowercase());
    gh.search(&[node(
        300,
        "Fix the parser",
        &branch,
        "OPEN",
        VIEWER,
        &later(1),
    )]);

    state.poll_pull_requests();

    let views = linked(&state, &id).await;
    assert_eq!(views[0]["url"], "https://github.com/acme/widgets/pull/300");
    assert_eq!(views[0]["status"]["state"], "open");
    let history = json_of(&state, &format!("/api/projects/test/tasks/{id}/history")).await;
    assert_eq!(history[0]["revision"]["agent"], "openplan-daemon");

    unlink(&state, &id, "https://github.com/acme/widgets/pull/300").await;
    let mut updated = node(300, "Fix the parser", &branch, "OPEN", VIEWER, &later(1));
    updated["updatedAt"] = json!(later(2));
    gh.search(&[updated]);
    state.poll_pull_requests();

    assert_eq!(linked(&state, &id).await, Value::Null);
}

#[tokio::test]
async fn a_pull_request_of_another_author_does_not_link() {
    let gh = FakeGh::new();
    let (_dir, state) = github_state(polling(&gh.program()));
    let id = create_in(&state, PROJECT, json!({ "title": "Parser" })).await;
    state.poll_pull_requests();
    gh.search(&[node(
        300,
        &format!("Fix ({id})"),
        "x",
        "OPEN",
        "ben",
        &later(1),
    )]);

    state.poll_pull_requests();

    assert_eq!(linked(&state, &id).await, Value::Null);
}

#[tokio::test]
async fn the_cursor_outlives_the_daemon() {
    let gh = FakeGh::new();
    let home = tempfile::tempdir().unwrap();
    let cache_file = home.path().join(op_server::CACHE_FILE);
    std::fs::write(
        &cache_file,
        json!({ "github.com/acme/widgets": { "cursor": {
            "updated": "2026-01-01T00:00:00Z", "since": "2026-01-01T00:00:00Z", "considered": {}
        }}})
        .to_string(),
    )
    .unwrap();
    let (_dir, state) = github_state(PullRequestPolling {
        cache_file: Some(cache_file.clone()),
        ..polling(&gh.program())
    });
    let id = create_in(&state, PROJECT, json!({ "title": "Parser" })).await;
    gh.search(&[node(
        300,
        &format!("Fix ({id})"),
        "x",
        "OPEN",
        VIEWER,
        "2026-02-01T00:00:00Z",
    )]);

    state.poll_pull_requests();

    assert_eq!(
        linked(&state, &id).await[0]["url"],
        "https://github.com/acme/widgets/pull/300",
        "a pull request opened while the daemon was down links"
    );
    let kept: Value = serde_json::from_slice(&std::fs::read(&cache_file).unwrap()).unwrap();
    assert_eq!(
        kept["github.com/acme/widgets"]["cursor"]["updated"],
        "2026-02-01T00:00:00Z"
    );
    assert!(
        gh.calls()[0].contains("updated:>=2025-12-31T23:55:00+00:00"),
        "the search starts five minutes before the cursor: {:?}",
        gh.calls()
    );
}

#[tokio::test]
async fn the_states_of_the_last_run_show_with_no_state_query() {
    let gh = FakeGh::new();
    let home = tempfile::tempdir().unwrap();
    let cached = PullRequestPolling {
        cache_file: Some(home.path().join(op_server::CACHE_FILE)),
        ..polling(&gh.program())
    };
    let (dir, state) = github_state(cached.clone());
    let id = create_in(
        &state,
        PROJECT,
        json!({ "title": "Parser", "pull_requests": [OWN] }),
    )
    .await;
    gh.states(json!({
        "r0": { "p214": node(214, "Fix the parser", "x", "MERGED", "ben", "2026-01-01T00:00:00Z") },
    }));
    state.poll_pull_requests();
    drop(state);

    let state = AppState::new([op_server::Project::open(
        PROJECT,
        op_server::Location::find(dir.path(), None).unwrap(),
    )
    .unwrap()])
    .with_pull_request_polling(cached);
    gh.states(json!({}));
    let before = gh.calls().len();
    state.poll_pull_requests();

    assert_eq!(linked(&state, &id).await[0]["status"]["state"], "merged");
    let calls = &gh.calls()[before..];
    assert_eq!(calls.len(), 1, "only the change query: {calls:?}");
    assert!(calls[0].contains("search("), "{calls:?}");
}

#[tokio::test]
async fn the_state_query_goes_in_batches() {
    let gh = FakeGh::new();
    let (_dir, state) = github_state(polling(&gh.program()));
    let addresses: Vec<String> = (1..=120)
        .map(|number| format!("https://github.com/acme/widgets/pull/{number}"))
        .collect();
    let id = create_in(
        &state,
        PROJECT,
        json!({ "title": "Parser", "pull_requests": addresses }),
    )
    .await;
    let pull_requests: serde_json::Map<String, Value> = (1..=120)
        .map(|number| {
            (
                format!("p{number}"),
                node(number, "T", "x", "OPEN", "ben", "2026-01-01T00:00:00Z"),
            )
        })
        .collect();
    gh.states(json!({ "r0": pull_requests }));

    state.poll_pull_requests();

    let queries: Vec<String> = gh
        .calls()
        .into_iter()
        .filter(|call| !call.contains("search("))
        .collect();
    assert_eq!(queries.len(), 3, "120 pull requests in batches of 50");
    assert!(
        queries
            .iter()
            .all(|query| query.matches("pullRequest(").count() <= 50)
    );
    let views = linked(&state, &id).await;
    assert!(
        views
            .as_array()
            .unwrap()
            .iter()
            .all(|view| view["status"]["state"] == "open"),
        "{views}"
    );
}

#[tokio::test]
async fn with_no_gh_the_links_have_no_state_and_the_project_has_a_fault() {
    let (_dir, state) = github_state(polling(Path::new("/nonexistent/gh")));
    let id = create_in(&state, PROJECT, json!({ "title": "Parser" })).await;
    state.poll_pull_requests();
    assert_eq!(
        json_of(&state, "/api/faults").await,
        json!([]),
        "no link, no fault"
    );

    let response = send(
        &state,
        "PATCH",
        &format!("/api/projects/test/tasks/{id}"),
        Some(json!({ "add_pull_requests": [OWN] })),
    )
    .await;
    assert!(response.status().is_success());
    state.poll_pull_requests();

    assert_eq!(linked(&state, &id).await[0].get("status"), None);
    let faults = json_of(&state, "/api/faults").await;
    assert_eq!(faults[0]["kind"], "pull_request_states");
    assert!(
        faults[0]["message"]
            .as_str()
            .unwrap()
            .contains("`gh auth login`"),
        "{faults}"
    );
}
