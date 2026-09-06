use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::response::Response;
use http_body_util::BodyExt;
use op_server::{AppState, Project, app};
use serde_json::{Value, json};
use tower::ServiceExt;

const PROJECT: &str = "test";

fn git(dir: &std::path::Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .current_dir(dir)
        .args(args)
        .status()
        .expect("git must be installed for this test");
    assert!(status.success(), "git {args:?} failed");
}

async fn send(state: &AppState, method: &str, uri: &str, body: Option<Value>) -> Response {
    let builder = Request::builder().method(method).uri(uri);
    let request = match body {
        Some(value) => builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_vec(&value).unwrap()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    };
    app(state.clone()).oneshot(request).await.unwrap()
}

async fn body_json(response: Response) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

// A repository whose serve root sits on the default branch, with the branch started the way the
// daemon starts it. Its worktree is what makes the branch writable.
fn with_rolling_updates() -> (tempfile::TempDir, AppState) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "t@example.com"]);
    git(root, &["config", "user.name", "Test"]);
    std::fs::create_dir_all(root.join(".plan/tasks")).unwrap();
    std::fs::write(root.join(".plan/config.toml"), "abbreviation = \"OPP\"\n").unwrap();
    git(root, &["add", "-A"]);
    git(root, &["commit", "-qm", "init"]);
    let store = op_store::Store::discover(root).unwrap();
    let repo = op_git::Repo::discover(root).unwrap();
    let config = op_store::Config {
        abbreviation: store.abbreviation(),
        default_branch: None,
    };
    let state = AppState::new([Project::new(
        PROJECT,
        root.to_path_buf(),
        repo,
        store,
        &config,
    )]);
    state.start_watchers();
    install_driver(root);
    (dir, state)
}

// The daemon registers its own binary as the driver. A test binary is not that, so these tests
// register a driver of their own: without one, git resolves every conflict silently to `ours`.
fn install_driver(root: &std::path::Path) {
    let script = root.join(".git/driver.sh");
    std::fs::write(
        &script,
        "#!/bin/sh\nexec git merge-file -L ours -L base -L theirs \"$2\" \"$1\" \"$3\"\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    git(
        root,
        &[
            "config",
            "merge.openplan.driver",
            &format!("{} %O %A %B", script.display()),
        ],
    );
}

fn bare_remote(root: &std::path::Path) -> std::path::PathBuf {
    let remote = root.parent().unwrap().join("remote.git");
    git(root, &["init", "-q", "--bare", remote.to_str().unwrap()]);
    git(root, &["remote", "add", "origin", remote.to_str().unwrap()]);
    remote
}

// The same section of the same task changed on the default branch and on the rolling-updates
// branch, so the rebase stops with markers in the worktree.
fn stop_the_rebase(root: &std::path::Path) {
    let repo = op_git::Repo::discover(root).unwrap();
    let on_main = root.join(".plan/tasks/00001-t.md");
    let on_rolling = repo
        .rolling_updates_worktree()
        .join(".plan/tasks/00001-t.md");
    let task = "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# T\n\n## Plan\n\nbase\n";
    std::fs::write(&on_main, task).unwrap();
    git(root, &["add", "-A"]);
    git(root, &["commit", "-qm", "a task"]);
    repo.rolling_updates_rebase("main").unwrap();
    std::fs::write(&on_rolling, task.replace("base", "rolling")).unwrap();
    repo.rolling_updates_commit("an edit for later").unwrap();
    std::fs::write(&on_main, task.replace("base", "main")).unwrap();
    git(root, &["commit", "-qam", "an edit on main"]);
    assert!(matches!(
        repo.rolling_updates_rebase("main").unwrap(),
        op_git::Rebased::Blocked { .. }
    ));
}

async fn create(state: &AppState, title: &str, query: &str) -> Value {
    let uri = format!("/api/projects/{PROJECT}/tasks{query}");
    let response = send(state, "POST", &uri, Some(json!({ "title": title }))).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    body_json(response).await
}

async fn waiting(state: &AppState) -> Value {
    let uri = format!("/api/projects/{PROJECT}/rolling-updates");
    let response = send(state, "GET", &uri, None).await;
    assert_eq!(response.status(), StatusCode::OK);
    body_json(response).await
}

#[test]
fn starting_the_daemon_gives_the_repository_a_rolling_updates_branch() {
    let (dir, _state) = with_rolling_updates();

    let rolling = dir.path().join(".git/openplan-rolling-updates");
    assert!(rolling.join(".plan").is_dir());
    assert!(rolling.join(".gitattributes").is_file());
    let attributes = std::fs::read_to_string(rolling.join(".gitattributes")).unwrap();
    assert!(attributes.contains("merge=openplan"), "{attributes}");
}

#[tokio::test]
async fn a_write_that_names_the_rolling_updates_branch_lands_there() {
    let (dir, state) = with_rolling_updates();

    create(
        &state,
        "An edit for later",
        "?branch=openplan/rolling-updates",
    )
    .await;

    assert!(
        !dir.path()
            .join(".plan/tasks")
            .read_dir()
            .unwrap()
            .any(|_| true)
    );
    let rolling = dir.path().join(".git/openplan-rolling-updates/.plan/tasks");
    assert_eq!(rolling.read_dir().unwrap().count(), 1);
}

#[tokio::test]
async fn a_write_that_names_no_branch_still_lands_on_the_default_branch() {
    let (dir, state) = with_rolling_updates();

    create(&state, "A plain edit", "").await;

    assert_eq!(
        dir.path().join(".plan/tasks").read_dir().unwrap().count(),
        1
    );
}

#[tokio::test]
async fn a_write_that_names_another_branch_still_lands_there() {
    let (dir, state) = with_rolling_updates();
    git(dir.path(), &["branch", "feature"]);
    let worktree = dir.path().join(".git/feature-worktree");
    git(
        dir.path(),
        &[
            "worktree",
            "add",
            "-q",
            worktree.to_str().unwrap(),
            "feature",
        ],
    );

    create(&state, "A branch edit", "?branch=feature").await;

    assert_eq!(worktree.join(".plan/tasks").read_dir().unwrap().count(), 1);
}

#[tokio::test]
async fn the_route_lists_what_is_waiting_and_publish_pushes_it_to_the_remote() {
    let (dir, state) = with_rolling_updates();
    let remote = bare_remote(dir.path());
    create(
        &state,
        "An edit for later",
        "?branch=openplan/rolling-updates",
    )
    .await;
    let repo = op_git::Repo::discover(dir.path()).unwrap();
    repo.rolling_updates_commit("Rolling task updates").unwrap();
    let main = repo.branch_commit("main").unwrap();

    let held = waiting(&state).await;
    assert_eq!(held["pending"].as_array().unwrap().len(), 1);
    assert_eq!(held["pending"][0]["task"]["title"], "An edit for later");
    assert!(held["conflict"].is_null());

    let uri = format!("/api/projects/{PROJECT}/rolling-updates/publish");
    let response = send(&state, "POST", &uri, None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let published = body_json(response).await;

    let branch = repo.rolling_updates_remote_branch();
    assert_eq!(published["remote"], "origin");
    assert_eq!(published["branch"], branch);
    assert_eq!(
        published["commit"],
        repo.branch_commit(op_git::ROLLING_UPDATES_BRANCH).unwrap()
    );
    assert_eq!(
        String::from_utf8_lossy(
            &std::process::Command::new("git")
                .current_dir(&remote)
                .args(["rev-parse", &branch])
                .output()
                .unwrap()
                .stdout
        )
        .trim(),
        published["commit"].as_str().unwrap()
    );
    // Publish never moves the default branch, here or on the remote. A person merges the request.
    assert_eq!(repo.branch_commit("main").unwrap(), main);
    assert_eq!(
        dir.path().join(".plan/tasks").read_dir().unwrap().count(),
        0
    );
}

// The branch always carries its own `.gitattributes` commit, so "nothing to publish" cannot mean
// "the tips match". A reachable remote leaves the refusal as the only reason this can fail.
#[tokio::test]
async fn publish_refuses_when_the_branch_holds_no_task_the_default_branch_lacks() {
    let (dir, state) = with_rolling_updates();
    bare_remote(dir.path());

    let uri = format!("/api/projects/{PROJECT}/rolling-updates/publish");
    let response = send(&state, "POST", &uri, None).await;

    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert!(
        body_json(response)
            .await
            .to_string()
            .contains("no task the default branch lacks")
    );
}

#[tokio::test]
async fn publish_refuses_while_a_conflict_holds_the_rolling_updates_branch() {
    let (dir, state) = with_rolling_updates();
    stop_the_rebase(dir.path());

    let uri = format!("/api/projects/{PROJECT}/rolling-updates/publish");
    let response = send(&state, "POST", &uri, None).await;

    assert_eq!(response.status(), StatusCode::CONFLICT);
    let held = waiting(&state).await;
    assert_eq!(held["conflict"]["files"][0], ".plan/tasks/00001-t.md");
    assert!(
        held["conflict"]["worktree"]
            .as_str()
            .unwrap()
            .ends_with("openplan-rolling-updates")
    );
}

// The route reads the rebase state from git on every call. A person who finishes the rebase in the
// worktree tells this process nothing, so a conflict the daemon remembered would never clear.
#[tokio::test]
async fn a_conflict_a_person_resolved_stops_being_reported() {
    let (dir, state) = with_rolling_updates();
    stop_the_rebase(dir.path());
    assert!(!waiting(&state).await["conflict"].is_null());

    git(
        &dir.path().join(".git/openplan-rolling-updates"),
        &["rebase", "--abort"],
    );

    assert!(waiting(&state).await["conflict"].is_null());
}
