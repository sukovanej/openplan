mod common;

use std::path::Path;

use axum::http::StatusCode;
use common::*;
use op_api::BackendKind;
use op_server::{AppState, Project, ProjectEntry, ProjectRegistry, REGISTRY_FILE};
use serde_json::{Value, json};

async fn board_rows(state: &AppState, uri: &str) -> Vec<(String, String, u64)> {
    json_of(state, uri).await["groups"]
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

fn two_local(alpha: &str, beta: &str) -> (tempfile::TempDir, tempfile::TempDir, AppState) {
    let alpha_dir = tempfile::tempdir().unwrap();
    let beta_dir = tempfile::tempdir().unwrap();
    let state = AppState::new([
        local_project("alpha", alpha_dir.path(), alpha),
        local_project("beta", beta_dir.path(), beta),
    ]);
    (alpha_dir, beta_dir, state)
}

fn with_registry(home: &Path, projects: impl IntoIterator<Item = Project>) -> AppState {
    AppState::new(projects).with_registry(home.join(REGISTRY_FILE))
}

async fn register(state: &AppState, body: Value) -> (StatusCode, Value) {
    let response = send(state, "POST", "/api/projects", Some(body)).await;
    let status = response.status();
    (status, body_json(response).await)
}

fn project_view<'a>(listed: &'a Value, name: &str) -> &'a Value {
    listed
        .as_array()
        .unwrap()
        .iter()
        .find(|view| view["name"] == name)
        .unwrap_or_else(|| panic!("{name} is listed: {listed}"))
}

fn break_config(state: &AppState, name: &str) {
    let project = state.project(name).unwrap();
    project
        .tracker()
        .backend()
        .commit(project.machine(), &mut |_| {
            Ok(op_backend::Edit::new(
                "Break the config",
                vec![op_backend::Op::put("config.toml", "abbreviation = 7\n")],
            ))
        })
        .unwrap();
    project.reload();
}

fn write_config(state: &AppState, name: &str, abbreviation: &str) {
    let project = state.project(name).unwrap();
    let text = format!("abbreviation = \"{abbreviation}\"\n");
    project
        .tracker()
        .backend()
        .commit(project.machine(), &mut |_| {
            Ok(op_backend::Edit::new(
                "Write the config",
                vec![op_backend::Op::put("config.toml", text.as_str())],
            ))
        })
        .unwrap();
    project.reload();
}

// Two directories, one daemon. They share nothing: not the id space, not the abbreviation, and not
// the index.
#[tokio::test]
async fn two_projects_interleave_and_allocate_ids_independently() {
    let (_alpha, _beta, state) = two_local("AAA", "BBB");

    let first = create_in(&state, "alpha", json!({ "title": "alpha one" })).await;
    let second = create_in(&state, "beta", json!({ "title": "beta one" })).await;
    let third = create_in(&state, "alpha", json!({ "title": "alpha two" })).await;
    assert_eq!((first.as_str(), second.as_str()), ("AAA-1", "BBB-1"));
    assert_eq!(third, "AAA-2");

    let listed = json_of(&state, "/api/projects/alpha/tasks").await;
    let titles: Vec<&str> = listed
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["title"].as_str().unwrap())
        .collect();
    assert_eq!(titles, vec!["alpha one", "alpha two"]);

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
        "AAA is not a key beta issues"
    );
}

// Two projects can use the same abbreviation, so a merged board keyed on the id alone would fold
// their tasks into one row, and nest a child under a parent from the other project.
#[tokio::test]
async fn the_merged_board_keeps_two_projects_that_share_an_abbreviation_apart() {
    let (_alpha, _beta, state) = two_local("APP", "APP");

    assert_eq!(
        create_in(&state, "alpha", json!({ "title": "alpha one" })).await,
        "APP-1"
    );
    assert_eq!(
        create_in(&state, "beta", json!({ "title": "beta one" })).await,
        "APP-1"
    );
    let child = create_in(
        &state,
        "beta",
        json!({ "title": "beta two", "parent": "APP-1" }),
    )
    .await;
    assert_eq!(child, "APP-2");

    let board = json_of(&state, "/api/board").await;
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
async fn a_merged_board_and_a_merged_search_span_a_local_and_a_git_project() {
    let local = tempfile::tempdir().unwrap();
    let git = tempfile::tempdir().unwrap();
    repository(git.path());
    let state = AppState::new([
        local_project("alpha", local.path(), "AAA"),
        git_project("beta", git.path(), "BBB"),
    ]);
    create_in(&state, "alpha", json!({ "title": "Shared word" })).await;
    create_in(&state, "beta", json!({ "title": "Shared word" })).await;

    let mut rows = board_rows(&state, "/api/board").await;
    rows.sort();
    assert_eq!(
        rows,
        vec![
            ("alpha".to_owned(), "AAA-1".to_owned(), 0),
            ("beta".to_owned(), "BBB-1".to_owned(), 0)
        ]
    );
    let hits = json_of(&state, "/api/search?q=shared").await;
    assert_eq!(hits.as_array().unwrap().len(), 2, "{hits}");

    let listed = json_of(&state, "/api/projects").await;
    let alpha = project_view(&listed, "alpha");
    assert_eq!(alpha["backend"], "local");
    assert_eq!(alpha["abbreviation"], "AAA");
    assert!(alpha.get("git_common_dir").is_none());
    let beta = project_view(&listed, "beta");
    assert_eq!(beta["backend"], "git");
    assert_eq!(beta["abbreviation"], "BBB");
    assert_eq!(
        beta["git_common_dir"],
        git.path()
            .join(".git")
            .canonicalize()
            .unwrap()
            .display()
            .to_string()
    );
    assert!(
        beta.get("sync").is_none(),
        "a repository with no remote has nothing to sync"
    );
}

// A project that cannot read its config is not a project with no tasks. It says why on
// `/api/projects`, and it leaves the merged board without taking the other project's rows down.
#[tokio::test]
async fn a_broken_config_demotes_one_project_and_leaves_the_other_serving() {
    let (_alpha, _beta, state) = two_local("AAA", "BBB");
    create_in(&state, "alpha", json!({ "title": "alpha one" })).await;
    create_in(&state, "beta", json!({ "title": "beta one" })).await;
    assert_eq!(board_rows(&state, "/api/board").await.len(), 2);

    break_config(&state, "alpha");

    let listed = json_of(&state, "/api/projects").await;
    let alpha = project_view(&listed, "alpha");
    assert_eq!(alpha["status"]["state"], "error");
    let reason = alpha["status"]["reason"].as_str().unwrap();
    assert!(reason.contains("abbreviation"), "{reason}");
    assert_eq!(project_view(&listed, "beta")["status"]["state"], "ok");
    assert_eq!(
        board_rows(&state, "/api/board").await,
        vec![("beta".to_owned(), "BBB-1".to_owned(), 0)]
    );

    write_config(&state, "alpha", "AAA");
    let listed = json_of(&state, "/api/projects").await;
    assert_eq!(project_view(&listed, "alpha")["status"]["state"], "ok");
    assert_eq!(board_rows(&state, "/api/board").await.len(), 2);
}

// A project whose storage cannot be read says so on `/api/projects` instead of reading as healthy,
// and the next good read clears it.
#[tokio::test]
async fn a_project_whose_tasks_cannot_be_read_says_so() {
    let (dir, state) = git_state();
    create(&state, "one").await;
    let reference = dir.path().join(".git/refs/openplan/tasks");
    let tip = std::fs::read_to_string(&reference).unwrap();

    std::fs::write(&reference, "0123456789012345678901234567890123456789\n").unwrap();
    project(&state).reload();
    let listed = json_of(&state, "/api/projects").await;
    assert_eq!(listed[0]["status"]["state"], "error", "{listed}");

    std::fs::write(&reference, tip).unwrap();
    project(&state).reload();
    let listed = json_of(&state, "/api/projects").await;
    assert_eq!(listed[0]["status"]["state"], "ok", "{listed}");
    assert_eq!(
        ids(&json_of(&state, "/api/projects/test/tasks").await),
        vec!["OPP-1"]
    );
}

// The merged board answers over every project, so "no project has rows" is an empty board rather
// than a refusal. The per-project board still refuses: it was asked about one project, and that
// project is the answer it cannot give.
#[tokio::test]
async fn the_merged_board_is_empty_rather_than_a_refusal_when_no_project_answers() {
    let empty = AppState::new([]);
    let response = send(&empty, "GET", "/api/board", None).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_json(response).await, json!({ "groups": [] }));

    let parent = tempfile::tempdir().unwrap();
    let root = parent.path().join("checkout");
    std::fs::create_dir(&root).unwrap();
    let state = AppState::new([local_project("alpha", &root, "AAA")]);
    create_in(&state, "alpha", json!({ "title": "alpha one" })).await;
    assert_eq!(board_rows(&state, "/api/board").await.len(), 1);

    std::fs::remove_dir_all(&root).unwrap();
    let project = state.project("alpha").unwrap();
    project.poll();
    project.poll();
    assert_eq!(
        json_of(&state, "/api/board").await,
        json!({ "groups": [] }),
        "the only project is demoted, and the merged board still answers"
    );
    assert_eq!(
        send(&state, "GET", "/api/projects/alpha/board", None)
            .await
            .status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "asked about that one project, the answer is still why it cannot serve"
    );
}

#[tokio::test]
async fn a_removed_root_demotes_the_project_and_a_restored_one_promotes_it() {
    let parent = tempfile::tempdir().unwrap();
    let beta = tempfile::tempdir().unwrap();
    let root = parent.path().join("checkout");
    std::fs::create_dir(&root).unwrap();
    let state = AppState::new([
        local_project("alpha", &root, "AAA"),
        local_project("beta", beta.path(), "BBB"),
    ]);
    let vanishing = state.project("alpha").unwrap();

    std::fs::remove_dir_all(&root).unwrap();
    assert!(!vanishing.poll(), "one miss is not yet a demotion");
    assert!(vanishing.poll(), "two misses in sequence demote");

    let refused = send(&state, "GET", "/api/projects/alpha/tasks", None).await;
    assert_eq!(refused.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(message_of(&body_json(refused).await).contains("no longer exists"));
    assert_eq!(
        send(&state, "GET", "/api/projects/beta/tasks", None)
            .await
            .status(),
        StatusCode::OK,
        "the daemon keeps serving; only the project with the missing root is demoted"
    );
    let listed = json_of(&state, "/api/projects").await;
    assert_eq!(project_view(&listed, "alpha")["status"]["state"], "error");

    std::fs::create_dir(&root).unwrap();
    assert!(vanishing.poll(), "the root is back");
    assert_eq!(
        send(&state, "GET", "/api/projects/alpha/tasks", None)
            .await
            .status(),
        StatusCode::OK
    );
}

// The abbreviation spells every key, so a new one re-keys every task at once.
#[tokio::test]
async fn a_new_abbreviation_re_keys_every_task() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new([local_project("alpha", dir.path(), "AAA")]);
    create_in(&state, "alpha", json!({ "title": "one" })).await;
    assert_eq!(
        ids(&json_of(&state, "/api/projects/alpha/tasks").await),
        vec!["AAA-1"]
    );

    write_config(&state, "alpha", "ZZZ");

    assert_eq!(
        ids(&json_of(&state, "/api/projects/alpha/tasks").await),
        vec!["ZZZ-1"]
    );
    let board = json_of(&state, "/api/projects/alpha/board").await;
    assert!(board.to_string().contains("ZZZ-1"), "{board}");
    assert_eq!(
        send(&state, "GET", "/api/projects/alpha/tasks/AAA-1", None)
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        json_of(&state, "/api/projects").await[0]["abbreviation"],
        "ZZZ"
    );
}

#[tokio::test]
async fn registering_a_directory_with_an_abbreviation_starts_its_tasks() {
    let home = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let state = with_registry(home.path(), []);

    let (status, view) =
        register(&state, json!({ "path": dir.path(), "abbreviation": "OPP" })).await;
    assert_eq!(status, StatusCode::CREATED, "{view}");
    assert_eq!(
        view["backend"], "local",
        "a directory outside git keeps its tasks in files"
    );
    assert_eq!(view["abbreviation"], "OPP");
    assert_eq!(view["status"]["state"], "ok");
    assert!(dir.path().join(".plan/config.toml").exists());

    let name = view["name"].as_str().unwrap().to_owned();
    let id = create_in(&state, &name, json!({ "title": "First" })).await;
    assert_eq!(id, "OPP-1");

    let registry = ProjectRegistry::read(&home.path().join(REGISTRY_FILE))
        .unwrap()
        .unwrap();
    assert_eq!(
        registry.entries(),
        [ProjectEntry {
            name,
            path: dir.path().canonicalize().unwrap(),
            backend: Some(BackendKind::Local),
        }]
    );
}

#[tokio::test]
async fn registering_a_repository_with_an_abbreviation_keeps_its_tasks_on_a_branch() {
    let home = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    repository(dir.path());
    let state = with_registry(home.path(), []);

    let (status, view) =
        register(&state, json!({ "path": dir.path(), "abbreviation": "OPP" })).await;
    assert_eq!(status, StatusCode::CREATED, "{view}");
    assert_eq!(view["backend"], "git");
    assert_eq!(view["abbreviation"], "OPP");
    assert!(!dir.path().join(".plan").exists());
    assert!(!git_output(dir.path(), &["rev-parse", op_backend_git::TASKS_NAME]).is_empty());

    let name = view["name"].as_str().unwrap().to_owned();
    assert_eq!(
        create_in(&state, &name, json!({ "title": "First" })).await,
        "OPP-1"
    );
}

#[tokio::test]
async fn a_repository_can_keep_its_tasks_in_local_files_when_asked() {
    let home = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    repository(dir.path());
    let state = with_registry(home.path(), []);

    let (status, view) = register(
        &state,
        json!({ "path": dir.path(), "backend": "local", "abbreviation": "OPP" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{view}");
    assert_eq!(view["backend"], "local");
    assert!(dir.path().join(".plan/.history.sqlite").exists());
}

#[tokio::test]
async fn starting_a_project_again_under_another_abbreviation_is_a_conflict() {
    let home = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let state = with_registry(home.path(), []);
    let (status, first) =
        register(&state, json!({ "path": dir.path(), "abbreviation": "OPP" })).await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, again) =
        register(&state, json!({ "path": dir.path(), "abbreviation": "OPP" })).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the same abbreviation starts nothing new"
    );
    assert_eq!(again["name"], first["name"]);

    let (status, refused) =
        register(&state, json!({ "path": dir.path(), "abbreviation": "ZZZ" })).await;
    assert_eq!(status, StatusCode::CONFLICT, "{refused}");
    assert!(message_of(&refused).contains("OPP"), "{refused}");
    assert_eq!(
        json_of(&state, "/api/projects").await[0]["abbreviation"],
        "OPP"
    );
}

#[tokio::test]
async fn registering_a_project_twice_answers_the_entry_it_already_has() {
    let home = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let state = with_registry(home.path(), []);

    let (status, entry) =
        register(&state, json!({ "path": dir.path(), "abbreviation": "AAA" })).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(entry["status"]["state"], "ok");

    let (status, again) = register(&state, json!({ "path": dir.path() })).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the CLI registers on its first write, and two of those can race"
    );
    assert_eq!(again["name"], entry["name"]);
    assert_eq!(
        json_of(&state, "/api/projects")
            .await
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn a_restarted_daemon_serves_what_it_registered() {
    let home = tempfile::tempdir().unwrap();
    let local = tempfile::tempdir().unwrap();
    let git = tempfile::tempdir().unwrap();
    repository(git.path());
    let state = with_registry(home.path(), []);
    for dir in [&local, &git] {
        let (status, view) =
            register(&state, json!({ "path": dir.path(), "abbreviation": "OPP" })).await;
        assert_eq!(status, StatusCode::CREATED);
        create_in(
            &state,
            view["name"].as_str().unwrap(),
            json!({ "title": "Kept" }),
        )
        .await;
    }
    drop(state);

    let registry = ProjectRegistry::read(&home.path().join(REGISTRY_FILE))
        .unwrap()
        .unwrap();
    let projects = op_server::open_projects(registry.entries());
    assert_eq!(projects.len(), 2);
    let restarted = AppState::new(projects);
    for view in json_of(&restarted, "/api/projects")
        .await
        .as_array()
        .unwrap()
    {
        let name = view["name"].as_str().unwrap();
        let tasks = json_of(&restarted, &format!("/api/projects/{name}/tasks")).await;
        assert_eq!(ids(&tasks), vec!["OPP-1"], "{name}");
    }
}

#[tokio::test]
async fn registering_a_path_that_cannot_be_served_names_the_missing_part() {
    let home = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let state = with_registry(home.path(), []);

    let (status, body) = register(&state, json!({ "path": dir.path() })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(message_of(&body).contains("openplan init"), "{body}");

    let (status, body) = register(
        &state,
        json!({ "path": dir.path(), "backend": "git", "abbreviation": "OPP" }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(message_of(&body).contains("git repository"), "{body}");

    let (status, body) =
        register(&state, json!({ "path": dir.path(), "abbreviation": "opp" })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        message_of(&body).contains("three uppercase letters"),
        "{body}"
    );

    let (status, body) = register(&state, json!({ "path": "relative/path" })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(message_of(&body).contains("absolute"), "{body}");

    assert!(
        !home.path().join(REGISTRY_FILE).exists(),
        "a refused path leaves no entry behind"
    );
    assert!(!dir.path().join(".plan").exists());
}

// Tasks in `.plan/` beside the code of a repository predate the tasks branch. The daemon names the
// command that moves them, rather than serve them as if nothing had changed.
#[tokio::test]
async fn a_repository_with_tasks_beside_the_code_needs_a_migration() {
    let home = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    repository(dir.path());
    std::fs::create_dir_all(dir.path().join(".plan/tasks")).unwrap();
    std::fs::write(
        dir.path().join(".plan/config.toml"),
        "abbreviation = \"OPP\"\n",
    )
    .unwrap();
    let state = with_registry(home.path(), []);

    let (status, body) = register(&state, json!({ "path": dir.path() })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(message_of(&body).contains("openplan migrate"), "{body}");
}

#[tokio::test]
async fn removing_a_project_stops_serving_it_and_leaves_its_files() {
    let home = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let state = with_registry(home.path(), [local_project("alpha", dir.path(), "AAA")]);
    create_in(&state, "alpha", json!({ "title": "one" })).await;

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
        "the daemon serves the tasks; it does not own them"
    );
}

// An entry the daemon could not open has no live project, and removing it spares the user an edit
// of the file the daemon owns.
#[tokio::test]
async fn removing_an_entry_the_daemon_could_not_open_clears_it_from_the_registry() {
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join(REGISTRY_FILE);
    let mut registry = ProjectRegistry::default();
    registry.insert(ProjectEntry {
        name: "gone".to_owned(),
        path: home.path().join("nowhere"),
        backend: None,
    });
    registry.write(&path).unwrap();
    let projects = op_server::open_projects(registry.entries());
    assert!(projects.is_empty());
    let state = AppState::new(projects).with_registry(path.clone());

    assert_eq!(
        send(&state, "DELETE", "/api/projects/gone", None)
            .await
            .status(),
        StatusCode::NO_CONTENT
    );
    assert!(
        ProjectRegistry::read(&path)
            .unwrap()
            .unwrap()
            .entries()
            .is_empty()
    );
}

#[tokio::test]
async fn renaming_a_project_moves_its_routes() {
    let home = tempfile::tempdir().unwrap();
    let alpha = tempfile::tempdir().unwrap();
    let beta = tempfile::tempdir().unwrap();
    let state = with_registry(
        home.path(),
        [
            local_project("alpha", alpha.path(), "AAA"),
            local_project("beta", beta.path(), "BBB"),
        ],
    );

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
    let registry = ProjectRegistry::read(&home.path().join(REGISTRY_FILE))
        .unwrap()
        .unwrap();
    assert!(registry.holds_name("work"));
    assert_eq!(registry.entries()[0].backend, Some(BackendKind::Local));

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

    let missing = send(
        &state,
        "PATCH",
        "/api/projects/ghost",
        Some(json!({ "name": "spirit" })),
    )
    .await;
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
}

// Every worktree of a repository reads the one tasks branch, so two worktrees are one project.
#[tokio::test]
async fn two_worktrees_of_one_repository_are_one_project() {
    let home = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    repository(dir.path());
    drop(git_project("main", dir.path(), "AAA"));
    git(dir.path(), &["commit", "-q", "--allow-empty", "-m", "init"]);
    let linked = dir.path().join("wt");
    git(
        dir.path(),
        &["worktree", "add", "-q", "-b", "feature", "wt"],
    );

    let entries = [
        ProjectEntry {
            name: "main".to_owned(),
            path: dir.path().to_path_buf(),
            backend: None,
        },
        ProjectEntry {
            name: "feature".to_owned(),
            path: linked.clone(),
            backend: None,
        },
    ];
    let opened = op_server::open_projects(&entries);
    assert_eq!(
        opened.iter().map(Project::name).collect::<Vec<_>>(),
        vec!["main"],
        "a hand-written registry naming two worktrees of one repository serves the first"
    );
    assert_eq!(opened[0].path, dir.path().canonicalize().unwrap());

    let state = AppState::new(opened).with_registry(home.path().join(REGISTRY_FILE));
    let (status, view) = register(&state, json!({ "path": linked })).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(view["name"], "main");
    assert_eq!(view["abbreviation"], "AAA");
}

// A name reaches the URL as one path segment. A name written by hand can be one no request can
// carry, and serving it would be serving a project nothing can reach.
#[tokio::test]
async fn a_hand_written_name_no_url_can_carry_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    drop(local_project("alpha", dir.path(), "AAA"));
    let entries = [
        ProjectEntry {
            name: "team/alpha".to_owned(),
            path: dir.path().to_path_buf(),
            backend: None,
        },
        ProjectEntry {
            name: String::new(),
            path: dir.path().to_path_buf(),
            backend: None,
        },
    ];
    assert!(op_server::open_projects(&entries).is_empty());
}

#[tokio::test]
async fn a_project_registered_over_http_answers_its_own_routes() {
    let home = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    drop(local_project("alpha", dir.path(), "AAA"));
    let state = with_registry(home.path(), []);

    let (status, registered) = register(&state, json!({ "path": dir.path() })).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(registered["abbreviation"], "AAA");
    let name = registered["name"].as_str().unwrap();
    assert_eq!(
        send(&state, "GET", &format!("/api/projects/{name}/tasks"), None)
            .await
            .status(),
        StatusCode::OK
    );
}

#[tokio::test]
async fn removing_a_project_leaves_the_others_serving() {
    let home = tempfile::tempdir().unwrap();
    let alpha = tempfile::tempdir().unwrap();
    let beta = tempfile::tempdir().unwrap();
    let state = with_registry(
        home.path(),
        [
            local_project("alpha", alpha.path(), "AAA"),
            local_project("beta", beta.path(), "BBB"),
        ],
    );

    send(&state, "DELETE", "/api/projects/alpha", None).await;
    assert_eq!(
        send(&state, "GET", "/api/projects/alpha/board", None)
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    let views = json_of(&state, "/api/projects").await;
    assert_eq!(views.as_array().unwrap().len(), 1, "{views}");
    assert_eq!(views[0]["name"], "beta");
    assert_eq!(views[0]["abbreviation"], "BBB");
    assert_eq!(
        send(&state, "GET", "/api/projects/beta/board", None)
            .await
            .status(),
        StatusCode::OK
    );
}

#[tokio::test]
async fn a_daemon_with_no_projects_still_serves() {
    let state = AppState::new([]);
    assert_eq!(
        send(&state, "GET", "/health", None).await.status(),
        StatusCode::OK
    );
    assert_eq!(json_of(&state, "/api/projects").await, json!([]));
    assert_eq!(json_of(&state, "/api/board").await, json!({ "groups": [] }));
    assert_eq!(
        json_of(&state, "/api/flow/drawing").await["width"],
        json!(0.0)
    );
}

// Membership changes are the daemon's own writes, so a state built from a fixed list has no file to
// keep in step and says so rather than drift from one.
#[tokio::test]
async fn a_state_with_no_registry_refuses_to_change_membership() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new([local_project("alpha", dir.path(), "AAA")]);

    let (status, _) = register(&state, json!({ "path": dir.path() })).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    for (method, body) in [("DELETE", None), ("PATCH", Some(json!({ "name": "work" })))] {
        assert_eq!(
            send(&state, method, "/api/projects/alpha", body)
                .await
                .status(),
            StatusCode::SERVICE_UNAVAILABLE,
            "{method}"
        );
    }
}
