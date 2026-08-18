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
    create_task(state, project, json!({ "title": title })).await
}

async fn create_task(state: &AppState, project: &str, body: Value) -> String {
    let response = send(
        state,
        "POST",
        &format!("/api/projects/{project}/tasks"),
        Some(body),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    body_json(response).await["id"].as_str().unwrap().to_owned()
}

async fn board_rows(state: &AppState, uri: &str) -> Vec<(String, String, u64)> {
    let response = send(state, "GET", uri, None).await;
    assert_eq!(response.status(), StatusCode::OK);
    body_json(response).await["groups"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|group| group["rows"].as_array().unwrap())
        .map(|row| {
            (
                row["task"]["project"].as_str().unwrap().to_owned(),
                row["task"]["id"].as_str().unwrap().to_owned(),
                row["depth"].as_u64().unwrap(),
            )
        })
        .collect()
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

// Two stores can commit the same abbreviation, so a merged board keyed on the id alone would fold
// their tasks into one row, and nest a child under a parent from the other project.
#[tokio::test]
async fn the_merged_board_keeps_two_stores_that_share_an_abbreviation_apart() {
    let alpha = tempfile::tempdir().unwrap();
    let beta = tempfile::tempdir().unwrap();
    repository(alpha.path(), "APP");
    repository(beta.path(), "APP");
    let state = AppState::new([open("alpha", alpha.path()), open("beta", beta.path())]);

    assert_eq!(create(&state, "alpha", "alpha one").await, "APP-1");
    assert_eq!(create(&state, "beta", "beta one").await, "APP-1");
    let child = create_task(
        &state,
        "beta",
        json!({ "title": "beta two", "parent": "APP-1" }),
    )
    .await;
    assert_eq!(child, "APP-2");

    let board = body_json(send(&state, "GET", "/api/board", None).await).await;
    let groups = board["groups"].as_array().unwrap();
    assert_eq!(groups.len(), 1, "every task here is backlog");
    let rows = groups[0]["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 3, "the shared key is two tasks, not one");

    let alpha_row = rows
        .iter()
        .find(|row| row["task"]["project"] == "alpha")
        .unwrap();
    assert_eq!(alpha_row["task"]["id"], "APP-1");
    assert_eq!(alpha_row["depth"], 0);
    assert_eq!(
        alpha_row["has_children"], false,
        "beta's child must not nest under alpha's task of the same key"
    );

    let nested: Vec<(&str, &str, u64)> = rows
        .iter()
        .filter(|row| row["task"]["project"] == "beta")
        .map(|row| {
            (
                row["task"]["id"].as_str().unwrap(),
                row["task"]["title"].as_str().unwrap(),
                row["depth"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        nested,
        vec![("APP-1", "beta one", 0), ("APP-2", "beta two", 1)]
    );
}

#[tokio::test]
async fn a_demoted_project_drops_out_of_the_merged_board() {
    let alpha = tempfile::tempdir().unwrap();
    let beta = tempfile::tempdir().unwrap();
    repository(alpha.path(), "AAA");
    repository(beta.path(), "BBB");
    let state = AppState::new([open("alpha", alpha.path()), open("beta", beta.path())]);
    create(&state, "alpha", "alpha one").await;
    create(&state, "beta", "beta one").await;
    assert_eq!(board_rows(&state, "/api/board").await.len(), 2);

    std::fs::write(alpha.path().join(".plan/config.toml"), "abbreviation = 7\n").unwrap();
    state.project("alpha").unwrap().reload_config();

    assert_eq!(
        board_rows(&state, "/api/board").await,
        vec![("beta".to_owned(), "BBB-1".to_owned(), 0)],
        "a project that cannot answer for its store leaves the board without failing it"
    );
}

// The merged board answers over every project, so "no project has rows" is an empty board rather
// than a refusal. The per-project board still 404s and 503s: it was asked about one project, and
// that project is the answer it cannot give.
#[tokio::test]
async fn the_merged_board_is_empty_rather_than_a_refusal_when_no_project_answers() {
    let empty = AppState::new([]);
    let response = send(&empty, "GET", "/api/board", None).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_json(response).await, json!({ "groups": [] }));

    let dir = tempfile::tempdir().unwrap();
    repository(dir.path(), "AAA");
    let state = AppState::new([open("alpha", dir.path())]);
    create(&state, "alpha", "alpha one").await;
    assert_eq!(board_rows(&state, "/api/board").await.len(), 1);

    std::fs::write(dir.path().join(".plan/config.toml"), "abbreviation = 7\n").unwrap();
    state.project("alpha").unwrap().reload_config();
    let response = send(&state, "GET", "/api/board", None).await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "the only project is demoted, and the merged board still answers"
    );
    assert_eq!(body_json(response).await, json!({ "groups": [] }));
    assert_eq!(
        send(&state, "GET", "/api/projects/alpha/board", None)
            .await
            .status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "asked about that one project, the answer is still why it cannot serve"
    );
}

// A repository that cannot be read is not a project with no tasks. It leaves the merged board, and
// it says why on `/api/projects` — otherwise the UI would show it as healthy and empty, which is
// the one thing the board must never claim about work somebody has.
#[tokio::test]
async fn a_project_whose_index_cannot_be_rebuilt_says_so_instead_of_reading_as_empty() {
    let alpha = tempfile::tempdir().unwrap();
    let beta = tempfile::tempdir().unwrap();
    repository(alpha.path(), "AAA");
    repository(beta.path(), "BBB");
    let state = AppState::new([open("alpha", alpha.path()), open("beta", beta.path())]);
    create(&state, "alpha", "alpha one").await;
    create(&state, "beta", "beta one").await;
    assert_eq!(board_rows(&state, "/api/board").await.len(), 2);

    let git = alpha.path().join(".git/objects");
    let moved = alpha.path().join(".git/objects-moved-away");
    std::fs::rename(&git, &moved).unwrap();

    assert_eq!(
        board_rows(&state, "/api/board").await,
        vec![("beta".to_owned(), "BBB-1".to_owned(), 0)],
        "one unreadable repository must not take the other project's rows down"
    );
    let listed = body_json(send(&state, "GET", "/api/projects", None).await).await;
    let entry = &listed.as_array().unwrap()[0];
    assert_eq!(entry["name"], "alpha");
    assert_eq!(
        entry["status"]["state"], "error",
        "a project that could not be read must not report as healthy"
    );

    // Nothing latches: the next read is what finds the repository readable again.
    std::fs::rename(&moved, &git).unwrap();
    assert_eq!(board_rows(&state, "/api/board").await.len(), 2);
    let listed = body_json(send(&state, "GET", "/api/projects", None).await).await;
    assert_eq!(listed.as_array().unwrap()[0]["status"]["state"], "ok");
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

// The daemon only ever writes a name `slug` produced. A name written by hand can be one no request
// can carry, and serving it would be serving a project nothing can reach.
#[tokio::test]
async fn a_hand_written_name_no_url_can_carry_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    repository(dir.path(), "AAA");
    let entries = [
        op_server::ProjectEntry {
            name: "team/alpha".to_owned(),
            path: dir.path().to_path_buf(),
        },
        op_server::ProjectEntry {
            name: String::new(),
            path: dir.path().to_path_buf(),
        },
    ];
    assert!(op_server::open_projects(&entries).is_empty());
}

// The matrix holds ids formatted with the abbreviation it was built under, and `Index::number_of`
// panics on a key the current abbreviation cannot parse. The dirty gate is what makes a matrix
// outlive its abbreviation, so changing one has to reopen the gate.
#[tokio::test]
async fn a_new_abbreviation_reopens_the_gate_rather_than_serving_the_old_keys() {
    let dir = tempfile::tempdir().unwrap();
    repository(dir.path(), "AAA");
    let state = AppState::new([open("alpha", dir.path())]);
    let project = state.project("alpha").unwrap();
    state.start_watchers();
    create(&state, "alpha", "one").await;

    let listed = body_json(send(&state, "GET", "/api/projects/alpha/tasks", None).await).await;
    assert_eq!(listed.as_array().unwrap()[0]["id"], "AAA-1");
    let before = rebuilds(&project);

    std::fs::write(
        dir.path().join(".plan/config.toml"),
        "abbreviation = \"ZZZ\"\n",
    )
    .unwrap();
    project.reload_config();
    assert!(
        rebuilds(&project) == before,
        "the reload itself does not walk the branches"
    );

    // Reading under the new abbreviation must rebuild, not render the matrix built under the old.
    let listed = body_json(send(&state, "GET", "/api/projects/alpha/tasks", None).await).await;
    assert!(
        rebuilds(&project) > before,
        "a new abbreviation is a change"
    );
    assert_eq!(listed.as_array().unwrap()[0]["id"], "ZZZ-1");
    // The board reaches `Index::number_of`, which panics on a key the abbreviation cannot parse.
    let board = send(&state, "GET", "/api/projects/alpha/board", None).await;
    assert_eq!(
        board.status(),
        StatusCode::OK,
        "the board renders rather than panicking on a key built under the old abbreviation"
    );
    assert!(
        body_json(board).await.to_string().contains("ZZZ-1"),
        "the board renders the new spelling"
    );
}

// A project registered over HTTP starts answering its own routes at once, without a restart.
#[tokio::test]
async fn a_project_registered_over_http_answers_its_own_routes() {
    let home = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    repository(dir.path(), "AAA");
    let state = AppState::new([]).with_registry(home.path().join("registry.toml"));

    let registered = body_json(
        send(
            &state,
            "POST",
            "/api/projects",
            Some(json!({ "path": dir.path() })),
        )
        .await,
    )
    .await;
    let name = registered["name"].as_str().unwrap().to_owned();
    assert_eq!(
        send(&state, "GET", &format!("/api/projects/{name}/tasks"), None)
            .await
            .status(),
        StatusCode::OK
    );
}

// Removing one project must not take the others down with it.
#[tokio::test]
async fn removing_a_project_leaves_the_others_serving() {
    let home = tempfile::tempdir().unwrap();
    let alpha = tempfile::tempdir().unwrap();
    let beta = tempfile::tempdir().unwrap();
    repository(alpha.path(), "AAA");
    repository(beta.path(), "BBB");
    let state = AppState::new([open("alpha", alpha.path()), open("beta", beta.path())])
        .with_registry(home.path().join("registry.toml"));

    send(&state, "DELETE", "/api/projects/alpha", None).await;
    assert_eq!(
        send(&state, "GET", "/api/projects/alpha/config", None)
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    let config = body_json(send(&state, "GET", "/api/projects/beta/config", None).await).await;
    assert_eq!(config["abbreviation"], "BBB");
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
    let merged = body_json(send(&state, "GET", "/api/board", None).await).await;
    assert_eq!(merged, json!({ "groups": [] }));
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

// The store's own config decides the merge target, so a change to it must move the baseline the
// whole matrix is measured from — without a restart, and for a branch nobody has checked out.
#[tokio::test]
async fn a_new_default_branch_in_the_config_moves_the_baseline() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    repository(root, "AAA");
    let task = root.join(".plan/tasks/00001-alpha.md");
    std::fs::write(
        &task,
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# A\n",
    )
    .unwrap();
    git(root, &["add", "-A"]);
    git(root, &["commit", "-qm", "add a"]);
    git(root, &["checkout", "-q", "-b", "dev"]);
    std::fs::write(
        &task,
        "---\nstatus: done\ncreated: 2026-01-01T00:00:00Z\n---\n# A\n",
    )
    .unwrap();
    git(root, &["add", "-A"]);
    git(root, &["commit", "-qm", "edit a on dev"]);
    git(root, &["checkout", "-q", "main"]);

    let state = AppState::new([open("alpha", root)]);
    assert_eq!(
        branch_names(&state).await,
        vec!["dev", "main"],
        "main is the autodetected baseline, and dev differs from it"
    );

    std::fs::write(
        root.join(".plan/config.toml"),
        "abbreviation = \"AAA\"\ndefault_branch = \"dev\"\n",
    )
    .unwrap();
    state.project("alpha").unwrap().reload_config();

    assert_eq!(
        branch_names(&state).await,
        vec!["dev"],
        "dev is the baseline now, so main carries nothing of its own"
    );
}

async fn branch_names(state: &AppState) -> Vec<String> {
    let response = send(state, "GET", "/api/projects/alpha/tasks", None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let items = body_json(response).await;
    let items = items.as_array().unwrap();
    assert_eq!(items.len(), 1, "one row per logical task: {items:?}");
    items[0]["branches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["branch"].as_str().unwrap().to_owned())
        .collect()
}
