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

// A repository whose serve root sits on the trunk, with the lane started the way the daemon starts
// it. The lane's worktree is what makes the ambient branch writable.
fn laned() -> (tempfile::TempDir, AppState) {
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

async fn create(state: &AppState, title: &str, query: &str) -> Value {
    let uri = format!("/api/projects/{PROJECT}/tasks{query}");
    let response = send(state, "POST", &uri, Some(json!({ "title": title }))).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    body_json(response).await
}

async fn sync(state: &AppState) -> Value {
    let uri = format!("/api/projects/{PROJECT}/sync");
    let response = send(state, "GET", &uri, None).await;
    assert_eq!(response.status(), StatusCode::OK);
    body_json(response).await
}

#[test]
fn starting_the_daemon_gives_the_repository_a_lane() {
    let (dir, _state) = laned();

    let lane = dir.path().join(".git/openplan-updates");
    assert!(lane.join(".plan").is_dir());
    assert!(lane.join(".gitattributes").is_file());
    let attributes = std::fs::read_to_string(lane.join(".gitattributes")).unwrap();
    assert!(attributes.contains("merge=openplan"), "{attributes}");
}

#[tokio::test]
async fn a_write_that_would_land_on_the_trunk_lands_on_the_lane() {
    let (dir, state) = laned();

    create(&state, "An ambient edit", "").await;

    assert!(
        !dir.path()
            .join(".plan/tasks")
            .read_dir()
            .unwrap()
            .any(|_| true)
    );
    let lane = dir.path().join(".git/openplan-updates/.plan/tasks");
    assert_eq!(lane.read_dir().unwrap().count(), 1);
}

#[tokio::test]
async fn a_write_that_names_another_branch_still_lands_there() {
    let (dir, state) = laned();
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
async fn sync_reports_what_the_lane_holds_and_publish_hands_it_to_the_trunk() {
    let (dir, state) = laned();
    create(&state, "An ambient edit", "").await;
    let repo = op_git::Repo::discover(dir.path()).unwrap();
    repo.lane_commit("Ambient task edits").unwrap();

    let status = sync(&state).await;
    assert_eq!(status["state"], "pending");
    assert_eq!(status["pending"].as_array().unwrap().len(), 1);
    assert_eq!(status["pending"][0]["task"]["title"], "An ambient edit");
    assert!(status["conflicted"].as_array().unwrap().is_empty());

    let uri = format!("/api/projects/{PROJECT}/publish");
    let response = send(&state, "POST", &uri, None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let published = body_json(response).await;
    assert_eq!(published["branch"], "main");

    assert_eq!(
        repo.branch_commit("main").unwrap(),
        repo.branch_commit(op_git::LANE_BRANCH).unwrap()
    );
    assert_eq!(
        dir.path().join(".plan/tasks").read_dir().unwrap().count(),
        1
    );
    assert_eq!(sync(&state).await["state"], "in_sync");
}

#[tokio::test]
async fn publish_refuses_while_a_conflict_holds_the_lane() {
    let (dir, state) = laned();
    let repo = op_git::Repo::discover(dir.path()).unwrap();
    let lane = dir.path().join(".git/openplan-updates");
    let task = "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# T\n\n## Plan\n\nbase\n";
    std::fs::write(dir.path().join(".plan/tasks/00001-t.md"), task).unwrap();
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-qm", "a task"]);
    repo.lane_rebase("main").unwrap();

    std::fs::write(
        lane.join(".plan/tasks/00001-t.md"),
        task.replace("base", "lane"),
    )
    .unwrap();
    repo.lane_commit("an ambient edit").unwrap();
    std::fs::write(
        dir.path().join(".plan/tasks/00001-t.md"),
        task.replace("base", "trunk"),
    )
    .unwrap();
    git(dir.path(), &["commit", "-qam", "a trunk edit"]);
    assert!(matches!(
        repo.lane_rebase("main").unwrap(),
        op_git::Rebased::Blocked { .. }
    ));

    let uri = format!("/api/projects/{PROJECT}/publish");
    let response = send(&state, "POST", &uri, None).await;

    assert_eq!(response.status(), StatusCode::CONFLICT);
    let status = sync(&state).await;
    assert_eq!(status["state"], "blocked");
    assert_eq!(status["conflicted"][0], ".plan/tasks/00001-t.md");
    assert!(
        status["worktree"]
            .as_str()
            .unwrap()
            .ends_with("openplan-updates")
    );
}

#[tokio::test]
async fn a_read_scoped_to_the_trunk_sees_what_the_lane_holds() {
    let (dir, state) = laned();
    create(&state, "An ambient edit", "").await;
    op_git::Repo::discover(dir.path())
        .unwrap()
        .lane_commit("Ambient task edits")
        .unwrap();

    let uri = format!("/api/projects/{PROJECT}/tasks?branch=main&fresh=true");
    let listed = body_json(send(&state, "GET", &uri, None).await).await;

    assert_eq!(listed.as_array().unwrap().len(), 1);
    assert_eq!(listed[0]["title"], "An ambient edit");
    assert_eq!(listed[0]["headline"], op_git::LANE_BRANCH);
    assert_eq!(listed[0]["write_target"]["branch"], op_git::LANE_BRANCH);
}
