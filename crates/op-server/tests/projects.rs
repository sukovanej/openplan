use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::response::Response;
use http_body_util::BodyExt;
use op_server::{AppState, Project, app};
use serde_json::{Value, json};
use tower::ServiceExt;

// A git-backed store checked out on `main`, the shape the daemon always serves.
fn repository(dir: &std::path::Path, abbreviation: &str) {
    git(dir, &["init", "-q", "-b", "main"]);
    git(dir, &["config", "user.email", "t@example.com"]);
    git(dir, &["config", "user.name", "Test"]);
    std::fs::create_dir_all(dir.join(".plan/tasks")).unwrap();
    std::fs::write(
        dir.join(".plan/config.toml"),
        format!("abbreviation = \"{abbreviation}\"\n"),
    )
    .unwrap();
    git(dir, &["commit", "-q", "--allow-empty", "-m", "init"]);
}

fn git(root: &std::path::Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .args(args)
        .current_dir(root)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?}");
}

fn open(name: &str, path: &std::path::Path) -> Project {
    Project::open(name, path.to_path_buf()).unwrap()
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

async fn create(state: &AppState, project: &str, title: &str) -> String {
    let response = send(
        state,
        "POST",
        &format!("/api/projects/{project}/tasks"),
        Some(json!({ "title": title })),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    body_json(response).await["id"].as_str().unwrap().to_owned()
}

// Two repositories, one daemon. They share nothing: not the id space, not the abbreviation, and not
// the index.
#[tokio::test]
async fn two_projects_interleave_and_allocate_ids_independently() {
    let alpha = tempfile::tempdir().unwrap();
    let beta = tempfile::tempdir().unwrap();
    repository(alpha.path(), "AAA");
    repository(beta.path(), "BBB");
    let state = AppState::new([open("alpha", alpha.path()), open("beta", beta.path())]);

    let first = create(&state, "alpha", "alpha one").await;
    let second = create(&state, "beta", "beta one").await;
    let third = create(&state, "alpha", "alpha two").await;
    assert_eq!((first.as_str(), second.as_str()), ("AAA-1", "BBB-1"));
    assert_eq!(third, "AAA-2");

    let listed = body_json(send(&state, "GET", "/api/projects/alpha/tasks", None).await).await;
    let titles: Vec<&str> = listed
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["title"].as_str().unwrap())
        .collect();
    assert_eq!(titles, vec!["alpha one", "alpha two"]);

    // The same number lives in both, and each project resolves only its own.
    assert_eq!(
        send(&state, "GET", "/api/projects/beta/tasks/BBB-1", None)
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        send(&state, "GET", "/api/projects/beta/tasks/AAA-1", None)
            .await
            .status(),
        StatusCode::BAD_REQUEST,
        "AAA is not a key beta's store issues"
    );
}

#[tokio::test]
async fn a_broken_config_demotes_one_project_and_leaves_the_other_serving() {
    let alpha = tempfile::tempdir().unwrap();
    let beta = tempfile::tempdir().unwrap();
    repository(alpha.path(), "AAA");
    repository(beta.path(), "BBB");
    let state = AppState::new([open("alpha", alpha.path()), open("beta", beta.path())]);
    let broken = state.project("alpha").unwrap();

    std::fs::write(alpha.path().join(".plan/config.toml"), "abbreviation = 7\n").unwrap();
    broken.reload_config();

    let refused = send(&state, "GET", "/api/projects/alpha/tasks", None).await;
    assert_eq!(refused.status(), StatusCode::SERVICE_UNAVAILABLE);
    let message = body_json(refused).await["message"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(message.contains("abbreviation"), "{message}");

    assert_eq!(
        send(&state, "GET", "/api/projects/beta/tasks", None)
            .await
            .status(),
        StatusCode::OK,
        "one broken project must not take the others down"
    );

    // A demoted project is still registered, and says why it cannot answer.
    let listed = body_json(send(&state, "GET", "/api/projects", None).await).await;
    let alpha_entry = &listed.as_array().unwrap()[0];
    assert_eq!(alpha_entry["name"], "alpha");
    assert_eq!(alpha_entry["status"]["state"], "error");

    std::fs::write(
        alpha.path().join(".plan/config.toml"),
        "abbreviation = \"AAA\"\n",
    )
    .unwrap();
    broken.reload_config();
    assert_eq!(
        send(&state, "GET", "/api/projects/alpha/tasks", None)
            .await
            .status(),
        StatusCode::OK,
        "a restored config promotes the project again"
    );
}

#[tokio::test]
async fn a_removed_root_demotes_the_project_and_a_restored_one_promotes_it() {
    let alpha = tempfile::tempdir().unwrap();
    let beta = tempfile::tempdir().unwrap();
    repository(alpha.path(), "AAA");
    repository(beta.path(), "BBB");
    let root = alpha.path().join("checkout");
    std::fs::create_dir(&root).unwrap();
    repository(&root, "AAA");
    let state = AppState::new([open("alpha", &root), open("beta", beta.path())]);
    let vanishing = state.project("alpha").unwrap();

    std::fs::remove_dir_all(&root).unwrap();
    assert!(!vanishing.poll_root(), "one miss is not yet a demotion");
    assert!(vanishing.poll_root(), "two misses in sequence demote");

    assert_eq!(
        send(&state, "GET", "/api/projects/alpha/tasks", None)
            .await
            .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        send(&state, "GET", "/api/projects/beta/tasks", None)
            .await
            .status(),
        StatusCode::OK,
        "the daemon keeps serving; only the project with the missing root is demoted"
    );

    std::fs::create_dir(&root).unwrap();
    assert!(vanishing.poll_root(), "the root is back");
    assert_eq!(
        send(&state, "GET", "/api/projects/alpha/tasks", None)
            .await
            .status(),
        StatusCode::OK
    );
}

// Every read used to walk each local branch. With N projects and a merged board that cost is paid N
// times per request, so a project nothing has changed must not be walked again.
#[tokio::test]
async fn a_read_on_a_clean_project_skips_the_rebuild() {
    let dir = tempfile::tempdir().unwrap();
    repository(dir.path(), "AAA");
    let state = AppState::new([open("alpha", dir.path())]);
    let project = state.project("alpha").unwrap();
    // Without a live watcher nothing can invalidate the matrix, so the gate stays open.
    state.start_watchers();

    assert_eq!(rebuilds(&project), 0);
    send(&state, "GET", "/api/projects/alpha/tasks", None).await;
    let first = rebuilds(&project);
    assert_eq!(first, 1, "the first read has nothing to trust");

    send(&state, "GET", "/api/projects/alpha/board", None).await;
    send(&state, "GET", "/api/projects/alpha/tasks", None).await;
    assert_eq!(rebuilds(&project), first, "a clean project is not rebuilt");

    // A write rebuilds in all conditions, and leaves the project readable again without a walk.
    create(&state, "alpha", "one").await;
    let after_write = rebuilds(&project);
    assert!(after_write > first, "a write always rebuilds");

    project.mark_dirty();
    send(&state, "GET", "/api/projects/alpha/tasks", None).await;
    assert!(
        rebuilds(&project) > after_write,
        "a change reported by the watcher reopens the gate"
    );
}

fn rebuilds(project: &Project) -> u64 {
    project.index.lock().unwrap().rebuilds()
}

#[tokio::test]
async fn registering_a_repository_twice_answers_the_entry_it_already_has() {
    let home = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    repository(dir.path(), "AAA");
    let state = AppState::new([]).with_registry(home.path().join("registry.toml"));

    let created = send(
        &state,
        "POST",
        "/api/projects",
        Some(json!({ "path": dir.path() })),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let entry = body_json(created).await;
    assert_eq!(entry["status"]["state"], "ok");

    let again = send(
        &state,
        "POST",
        "/api/projects",
        Some(json!({ "path": dir.path() })),
    )
    .await;
    assert_eq!(
        again.status(),
        StatusCode::OK,
        "the CLI auto-registers on its first write, and two of those can race"
    );
    assert_eq!(body_json(again).await["name"], entry["name"]);

    let listed = body_json(send(&state, "GET", "/api/projects", None).await).await;
    assert_eq!(listed.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn registering_a_path_that_cannot_be_served_names_the_missing_part() {
    let home = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new([]).with_registry(home.path().join("registry.toml"));

    let no_repo = send(
        &state,
        "POST",
        "/api/projects",
        Some(json!({ "path": dir.path() })),
    )
    .await;
    assert_eq!(no_repo.status(), StatusCode::BAD_REQUEST);
    let message = body_json(no_repo).await["message"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(message.contains("git repository"), "{message}");

    git(dir.path(), &["init", "-q", "-b", "main"]);
    let no_store = send(
        &state,
        "POST",
        "/api/projects",
        Some(json!({ "path": dir.path() })),
    )
    .await;
    assert_eq!(no_store.status(), StatusCode::BAD_REQUEST);
    let message = body_json(no_store).await["message"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(message.contains(".plan"), "{message}");

    assert!(
        !home.path().join("registry.toml").exists(),
        "a refused path leaves no entry behind"
    );
}

#[tokio::test]
async fn removing_a_project_stops_serving_it_and_leaves_its_files() {
    let home = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    repository(dir.path(), "AAA");
    let registry = home.path().join("registry.toml");
    let state = AppState::new([open("alpha", dir.path())]).with_registry(registry.clone());
    create(&state, "alpha", "one").await;

    assert_eq!(
        send(&state, "DELETE", "/api/projects/alpha", None)
            .await
            .status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        send(&state, "GET", "/api/projects/alpha/tasks", None)
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        send(&state, "DELETE", "/api/projects/alpha", None)
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    assert!(
        dir.path().join(".plan/tasks/00001-one.md").exists(),
        "the daemon serves a repository; it does not own one"
    );
}

#[tokio::test]
async fn renaming_a_project_moves_its_routes() {
    let home = tempfile::tempdir().unwrap();
    let alpha = tempfile::tempdir().unwrap();
    let beta = tempfile::tempdir().unwrap();
    repository(alpha.path(), "AAA");
    repository(beta.path(), "BBB");
    let state = AppState::new([open("alpha", alpha.path()), open("beta", beta.path())])
        .with_registry(home.path().join("registry.toml"));

    let renamed = send(
        &state,
        "PATCH",
        "/api/projects/alpha",
        Some(json!({ "name": "work" })),
    )
    .await;
    assert_eq!(renamed.status(), StatusCode::OK);
    assert_eq!(body_json(renamed).await["name"], "work");

    assert_eq!(
        send(&state, "GET", "/api/projects/work/tasks", None)
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        send(&state, "GET", "/api/projects/alpha/tasks", None)
            .await
            .status(),
        StatusCode::NOT_FOUND
    );

    let taken = send(
        &state,
        "PATCH",
        "/api/projects/work",
        Some(json!({ "name": "beta" })),
    )
    .await;
    assert_eq!(taken.status(), StatusCode::CONFLICT);

    let unusable = send(
        &state,
        "PATCH",
        "/api/projects/work",
        Some(json!({ "name": "Not A Slug" })),
    )
    .await;
    assert_eq!(unusable.status(), StatusCode::BAD_REQUEST);
}

// A number is issued at most once per repository, and each project issues from its own counter.
// Two worktrees of one repository served as two projects would each mint the same number into a
// different `.plan` directory, and neither store could see the other's file.
#[tokio::test]
async fn two_worktrees_of_one_repository_are_one_project() {
    let home = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    repository(dir.path(), "AAA");
    let linked = dir.path().join("wt");
    git(
        dir.path(),
        &["worktree", "add", "-q", "-b", "feature", "wt"],
    );

    let entries = [
        op_server::ProjectEntry {
            name: "main".to_owned(),
            path: dir.path().to_path_buf(),
        },
        op_server::ProjectEntry {
            name: "feature".to_owned(),
            path: linked.clone(),
        },
    ];
    let opened = op_server::open_projects(&entries);
    assert_eq!(
        opened.iter().map(Project::name).collect::<Vec<_>>(),
        vec!["main"],
        "a hand-written registry naming two worktrees of one repository serves the first"
    );

    // The route that adds one answers with the project the repository already has.
    let state = AppState::new(opened).with_registry(home.path().join("registry.toml"));
    let response = send(
        &state,
        "POST",
        "/api/projects",
        Some(json!({ "path": linked })),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_json(response).await["name"], "main");
}

// Zero projects is a served state: the daemon answers, and says so, rather than refusing to run.
#[tokio::test]
async fn a_daemon_with_no_projects_still_serves() {
    let state = AppState::new([]);
    assert_eq!(
        send(&state, "GET", "/health", None).await.status(),
        StatusCode::OK
    );
    let listed = body_json(send(&state, "GET", "/api/projects", None).await).await;
    assert_eq!(listed, json!([]));
    assert_eq!(
        send(&state, "GET", "/api/tasks", None).await.status(),
        StatusCode::NOT_FOUND
    );
}

// Membership changes are the daemon's own writes, so a state built from a fixed list has no file to
// keep in step and says so rather than diverging from one silently.
#[tokio::test]
async fn a_state_with_no_registry_refuses_to_change_membership() {
    let dir = tempfile::tempdir().unwrap();
    repository(dir.path(), "AAA");
    let state = AppState::new([open("alpha", dir.path())]);

    let response = send(
        &state,
        "POST",
        "/api/projects",
        Some(json!({ "path": dir.path() })),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}
