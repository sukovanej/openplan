mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use common::*;
use http_body_util::BodyExt;
use op_server::{AppState, app};
use serde_json::{Value, json};
use tower::ServiceExt;

async fn get(uri: &str) -> axum::response::Response {
    let (_dir, state) = local_state();
    send(&state, "GET", uri, None).await
}

#[tokio::test]
async fn health_returns_ok() {
    let response = get("/health").await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"ok");
}

#[tokio::test]
async fn spa_index_served_with_charset() {
    let response = get("/").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "text/html; charset=utf-8"
    );
}

// The page names the hashed assets of one build, so a browser must ask for it again every time.
#[tokio::test]
async fn the_page_is_not_cached_and_a_client_route_falls_back_to_it() {
    let page = get("/").await;
    assert!(page.headers().get(header::CACHE_CONTROL).is_none());
    let page = page.into_body().collect().await.unwrap().to_bytes();

    let routed = get("/projects/test/tasks/OPP-1").await;
    assert_eq!(routed.status(), StatusCode::OK);
    assert_eq!(
        routed.headers().get(header::CONTENT_TYPE).unwrap(),
        "text/html; charset=utf-8"
    );
    assert_eq!(routed.into_body().collect().await.unwrap().to_bytes(), page);
}

#[tokio::test]
async fn health_reports_identity_when_set() {
    let info = op_api::DaemonInfo {
        pid: 4242,
        port: 9,
        version: "9.9.9".to_owned(),
        started_at: 5,
    };
    let (_dir, state) = local_state();
    let response = send(&state.with_health(info.clone()), "GET", "/health", None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let served: op_api::DaemonInfo = serde_json::from_slice(&body).unwrap();
    assert_eq!(served, info);
}

#[tokio::test]
async fn admin_shutdown_returns_ok_with_admin_header() {
    let (_dir, state) = local_state();
    let response = send_as(
        &state,
        "POST",
        "/admin/shutdown",
        None,
        &[(op_api::ADMIN_HEADER, "1")],
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn admin_shutdown_forbidden_without_admin_header() {
    let (_dir, state) = local_state();
    let response = send(&state, "POST", "/admin/shutdown", None).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn serve_stops_on_external_shutdown() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let (_dir, state) = local_state();
    let server = tokio::spawn(op_server::serve(listener, state, async move {
        let _ = rx.await;
    }));

    tx.send(()).unwrap();
    server.await.unwrap().unwrap();
}

#[tokio::test]
async fn tasks_crud_roundtrip() {
    for (_dir, state) in [local_state(), git_state()] {
        let id = create(&state, "Wire the parser").await;
        assert_eq!(id, "OPP-1", "the id is the key of the allocated number");

        let list = json_of(&state, "/api/projects/test/tasks").await;
        assert_eq!(ids(&list), vec!["OPP-1"]);
        assert_eq!(list[0]["project"], PROJECT);

        let view = json_of(&state, &format!("/api/projects/test/tasks/{id}")).await;
        assert_eq!(view["title"], "Wire the parser");
        assert_eq!(view["metadata"]["status"], "backlog");
        assert_eq!(view["description"], "");

        let patched = send(
            &state,
            "PATCH",
            &format!("/api/projects/test/tasks/{id}"),
            Some(json!({ "status": "in_progress" })),
        )
        .await;
        assert_eq!(patched.status(), StatusCode::OK);
        assert_eq!(
            body_json(patched).await["metadata"]["status"],
            "in_progress"
        );

        let deleted = send(
            &state,
            "DELETE",
            &format!("/api/projects/test/tasks/{id}"),
            None,
        )
        .await;
        assert_eq!(deleted.status(), StatusCode::NO_CONTENT);

        let gone = send(
            &state,
            "GET",
            &format!("/api/projects/test/tasks/{id}"),
            None,
        )
        .await;
        assert_eq!(gone.status(), StatusCode::NOT_FOUND);
    }
}

// The tasks of a git project live on their own branch, so a write leaves the checkout alone.
#[tokio::test]
async fn a_git_project_writes_its_tasks_to_the_tasks_branch_and_not_to_the_checkout() {
    let (dir, state) = git_state();
    create(&state, "Wire the parser").await;

    assert!(!dir.path().join(".plan").exists());
    assert!(git_output(dir.path(), &["status", "--porcelain"]).is_empty());
    let files = git_output(
        dir.path(),
        &["ls-tree", "-r", "--name-only", op_backend_git::TASKS_NAME],
    );
    assert!(
        files
            .lines()
            .any(|file| file == "tasks/00001-wire-the-parser.md"),
        "{files}"
    );
}

#[tokio::test]
async fn a_local_project_writes_its_tasks_to_plain_files() {
    let (dir, state) = local_state();
    create(&state, "Wire the parser").await;

    let text =
        std::fs::read_to_string(dir.path().join(".plan/tasks/00001-wire-the-parser.md")).unwrap();
    assert!(text.contains("# Wire the parser"), "{text}");
    assert!(dir.path().join(".plan/.history.sqlite").exists());
}

#[tokio::test]
async fn search_matches_the_body_and_names_the_project() {
    let (_dir, state) = local_state();
    create_in(
        &state,
        PROJECT,
        json!({ "title": "Wire the parser", "body": "It must accept a zeppelin." }),
    )
    .await;
    create(&state, "Paint the shed").await;

    for uri in [
        "/api/projects/test/search?q=ZEPPELIN",
        "/api/search?q=zeppelin",
    ] {
        let hits = json_of(&state, uri).await;
        let hits = hits.as_array().unwrap();
        assert_eq!(hits.len(), 1, "{uri}");
        assert_eq!(hits[0]["task"]["id"], "OPP-1", "{uri}");
        assert_eq!(hits[0]["task"]["project"], PROJECT, "{uri}");
    }
}

#[tokio::test]
async fn search_with_no_query_matches_nothing() {
    let (_dir, state) = local_state();
    create(&state, "Wire the parser").await;

    for uri in [
        "/api/projects/test/search",
        "/api/projects/test/search?q=",
        "/api/search?q=",
    ] {
        assert!(
            json_of(&state, uri).await.as_array().unwrap().is_empty(),
            "{uri}"
        );
    }
}

#[tokio::test]
async fn searching_an_unknown_project_is_404() {
    let (_dir, state) = local_state();
    let response = send(&state, "GET", "/api/projects/nope/search?q=a", None).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// A path segment no key could ever name is a bad request, not a missing task, whichever method
// asks.
#[tokio::test]
async fn routes_naming_something_that_is_not_an_id_are_400() {
    let (_dir, state) = local_state();
    for (method, path, body) in [
        ("GET", "", None),
        ("PATCH", "", Some(json!({ "status": "done" }))),
        ("DELETE", "", None),
        (
            "PUT",
            "/file",
            Some(json!({ "text": task_file("todo", "A", "") })),
        ),
        ("GET", "/history", None),
        ("GET", "/comments", None),
        ("GET", "/tree", None),
    ] {
        let uri = format!("/api/projects/test/tasks/ship-login-3d0c{path}");
        let response = send(&state, method, &uri, body).await;
        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "{method} {uri} on a non-id must be a bad request"
        );
    }
}

#[tokio::test]
async fn missing_task_routes_are_404_and_say_so_plainly() {
    let (_dir, state) = local_state();
    for (method, path, body) in [
        ("GET", "", None),
        ("PATCH", "", Some(json!({ "status": "done" }))),
        ("DELETE", "", None),
        (
            "PUT",
            "/file",
            Some(json!({ "text": task_file("todo", "A", "") })),
        ),
        ("GET", "/comments", None),
        ("GET", "/tree", None),
    ] {
        let uri = format!("/api/projects/test/tasks/OPP-99{path}");
        let response = send(&state, method, &uri, body).await;
        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "{method} {uri} on a missing id must 404"
        );
        assert_eq!(
            message_of(&body_json(response).await),
            "no such task: OPP-99",
            "{method} {uri}"
        );
    }
}

#[tokio::test]
async fn a_project_with_no_tasks_yet_refuses_a_new_task() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new([open(PROJECT, dir.path(), op_api::BackendKind::Local)]);

    assert!(
        json_of(&state, "/api/projects/test/tasks")
            .await
            .as_array()
            .unwrap()
            .is_empty()
    );
    let refused = send(
        &state,
        "POST",
        "/api/projects/test/tasks",
        Some(json!({ "title": "Too soon" })),
    )
    .await;
    assert_eq!(refused.status(), StatusCode::CONFLICT);
    assert!(message_of(&body_json(refused).await).contains("openplan init"));
}

#[tokio::test]
async fn patch_parent_null_clears_absent_leaves_id_sets() {
    let (dir, state) = local_state();
    seed(
        &state,
        &[
            ("tasks/00001-epic.md", &task_file("todo", "Epic", "")),
            (
                "tasks/00002-child.md",
                &task_file("todo", "Child", "parent: ./00001-epic.md\n"),
            ),
        ],
    );

    let untouched = send(
        &state,
        "PATCH",
        "/api/projects/test/tasks/OPP-2",
        Some(json!({ "status": "in_progress" })),
    )
    .await;
    assert_eq!(untouched.status(), StatusCode::OK);
    assert_eq!(body_json(untouched).await["metadata"]["parent"], "OPP-1");

    let cleared = send(
        &state,
        "PATCH",
        "/api/projects/test/tasks/OPP-2",
        Some(json!({ "parent": null })),
    )
    .await;
    assert_eq!(cleared.status(), StatusCode::OK);
    assert!(body_json(cleared).await.get("parent_title").is_none());
    let raw = std::fs::read_to_string(dir.path().join(".plan/tasks/00002-child.md")).unwrap();
    assert!(!raw.contains("parent"), "a cleared key must drop: {raw}");

    let set = send(
        &state,
        "PATCH",
        "/api/projects/test/tasks/OPP-2",
        Some(json!({ "parent": "OPP-1" })),
    )
    .await;
    assert_eq!(set.status(), StatusCode::OK);
    assert_eq!(body_json(set).await["metadata"]["parent"], "OPP-1");
    let raw = std::fs::read_to_string(dir.path().join(".plan/tasks/00002-child.md")).unwrap();
    assert!(
        raw.contains("parent: ./00001-epic.md"),
        "a reference is stored as the file it names: {raw}"
    );
}

#[tokio::test]
async fn board_groups_by_status_and_nests_same_status_children() {
    let (_dir, state) = local_state();
    seed(
        &state,
        &[
            ("tasks/00001-epic.md", &task_file("in_progress", "Epic", "")),
            (
                "tasks/00002-sub-open.md",
                &task_file(
                    "in_progress",
                    "Sub open",
                    "parent: ./00001-epic.md\nrank: m\n",
                ),
            ),
            (
                "tasks/00003-sub-todo.md",
                &task_file("todo", "Sub todo", "parent: ./00001-epic.md\n"),
            ),
        ],
    );

    let board = json_of(&state, "/api/projects/test/board").await;
    let groups = board["groups"].as_array().unwrap();
    let order: Vec<&str> = groups
        .iter()
        .map(|group| group["status"].as_str().unwrap())
        .collect();
    assert_eq!(order, vec!["in_progress", "todo"]);

    let in_progress = &groups[0]["rows"];
    assert_eq!(in_progress[0]["task"]["id"], "OPP-1");
    assert_eq!(in_progress[0]["depth"], 0);
    assert_eq!(in_progress[0]["has_children"], true);
    assert_eq!(in_progress[1]["task"]["id"], "OPP-2");
    assert_eq!(in_progress[1]["depth"], 1);

    let todo = &groups[1]["rows"];
    assert_eq!(todo[0]["task"]["id"], "OPP-3");
    assert_eq!(todo[0]["depth"], 0);
    assert_eq!(todo[0]["parent_title"], "Epic");
}

#[tokio::test]
async fn task_detail_carries_parent_title_children_and_resolved_refs() {
    let (_dir, state) = local_state();
    seed(
        &state,
        &[
            ("tasks/00001-epic.md", &task_file("todo", "Epic", "")),
            (
                "tasks/00002-child.md",
                &format!(
                    "{}\nblocks [[./00001-epic.md]] and [[OPP-1]], not [[OPP-99]] or [[1]].\n",
                    task_file("in_progress", "Child", "parent: ./00001-epic.md\nrank: m\n")
                ),
            ),
            (
                "tasks/00004-b.md",
                &task_file("todo", "B", "parent: ./00002-child.md\nrank: t\n"),
            ),
            (
                "tasks/00003-a.md",
                &task_file("todo", "A", "parent: ./00002-child.md\nrank: m\n"),
            ),
        ],
    );

    let detail = json_of(&state, "/api/projects/test/tasks/OPP-2").await;
    assert_eq!(detail["parent_title"], "Epic");

    let children = detail["children"].as_array().unwrap();
    assert_eq!(
        children
            .iter()
            .map(|child| child["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["OPP-3", "OPP-4"],
        "rank order"
    );
    assert_eq!(children[0]["title"], "A");

    let refs = detail["refs"].as_array().unwrap();
    assert_eq!(refs.len(), 1, "one task under two spellings: {refs:?}");
    assert_eq!(refs[0]["id"], "OPP-1");
    assert_eq!(refs[0]["title"], "Epic");

    let epic = json_of(&state, "/api/projects/test/tasks/OPP-1").await;
    assert!(epic.get("parent_title").is_none());
    assert_eq!(epic["children"][0]["id"], "OPP-2");
}

#[tokio::test]
async fn task_detail_carries_both_directions_of_its_dependencies() {
    let (_dir, state) = local_state();
    seed(
        &state,
        &[
            ("tasks/00001-design.md", &task_file("done", "Design", "")),
            (
                "tasks/00002-api.md",
                &task_file(
                    "todo",
                    "API",
                    "dependencies:\n- ./00003-schema.md\n- ./00001-design.md#Wire\n- ./00001-design.md#Shape\n- ./00099-gone.md\n",
                ),
            ),
            ("tasks/00003-schema.md", &task_file("done", "Schema", "")),
            (
                "tasks/00004-ship.md",
                &task_file("todo", "Ship", "rank: t\ndependencies:\n- ./00002-api.md\n"),
            ),
            (
                "tasks/00005-web.md",
                &task_file(
                    "todo",
                    "Web",
                    "rank: m\ndependencies:\n- ./00002-api.md#Wire\n",
                ),
            ),
        ],
    );

    let detail = json_of(&state, "/api/projects/test/tasks/OPP-2").await;
    let depends_on = detail["depends_on"].as_array().unwrap();
    assert_eq!(
        ids(&detail["depends_on"]),
        vec!["OPP-3", "OPP-1"],
        "file order; an entry that names no task drops out, and one task counts once"
    );
    assert_eq!(depends_on[0]["title"], "Schema");
    assert_eq!(depends_on[0]["status"], "done");
    assert_eq!(
        ids(&detail["blocks"]),
        vec!["OPP-5", "OPP-4"],
        "rank order, and a sectioned entry still reports the task it names"
    );

    let schema = json_of(&state, "/api/projects/test/tasks/OPP-3").await;
    assert!(schema.get("depends_on").is_none());
    assert_eq!(schema["blocks"][0]["id"], "OPP-2");

    let patched = send(
        &state,
        "PATCH",
        "/api/projects/test/tasks/OPP-2",
        Some(json!({ "status": "in_progress" })),
    )
    .await;
    let patched = body_json(patched).await;
    assert_eq!(patched["depends_on"][0]["id"], "OPP-3");
    assert_eq!(patched["blocks"][0]["id"], "OPP-5");
}

#[tokio::test]
async fn patch_preserves_unknown_frontmatter_keys() {
    let (dir, state) = local_state();
    seed(
        &state,
        &[(
            "tasks/00001-keep.md",
            &task_file("todo", "Keep", "estimate: 9\n"),
        )],
    );

    let response = send(
        &state,
        "PATCH",
        "/api/projects/test/tasks/OPP-1",
        Some(json!({ "status": "done" })),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let raw = std::fs::read_to_string(dir.path().join(".plan/tasks/00001-keep.md")).unwrap();
    assert!(
        raw.contains("estimate: 9"),
        "estimate must survive PATCH: {raw}"
    );
    assert!(raw.contains("status: done"), "{raw}");
}

#[tokio::test]
async fn create_with_an_unknown_parent_is_400() {
    let (_dir, state) = local_state();
    for parent in ["ghost", "OPP-7"] {
        let response = send(
            &state,
            "POST",
            "/api/projects/test/tasks",
            Some(json!({ "title": "Child", "parent": parent })),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{parent}");
    }
    assert!(
        json_of(&state, "/api/projects/test/tasks")
            .await
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn malformed_json_body_is_400() {
    let (_dir, state) = local_state();
    let request = Request::builder()
        .method("POST")
        .uri("/api/projects/test/tasks")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{ not json"))
        .unwrap();
    let response = app(state).oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test(flavor = "multi_thread")]
async fn concurrent_creates_never_share_an_id() {
    for (_dir, state) in [local_state(), git_state()] {
        let mut requests = Vec::new();
        for _ in 0..8 {
            let state = state.clone();
            requests.push(tokio::spawn(
                async move { create(&state, "Contended").await },
            ));
        }
        let mut created = Vec::new();
        for request in requests {
            created.push(request.await.unwrap());
        }
        let distinct: std::collections::HashSet<&String> = created.iter().collect();
        assert_eq!(distinct.len(), created.len(), "ids collided: {created:?}");
        assert_eq!(
            json_of(&state, "/api/projects/test/tasks")
                .await
                .as_array()
                .unwrap()
                .len(),
            created.len(),
            "every create landed as its own task"
        );
    }
}

// Another process can take the next number while the daemon's index still ends below it. The
// create reads the head it writes on, not the index.
#[tokio::test]
async fn create_takes_the_next_number_when_another_process_took_one() {
    for (_dir, state) in [local_state(), git_state()] {
        let project = project(&state);
        let bob = other_process(&project);
        assert_eq!(
            bob.create_task(&op_backend::Actor::new("Bob"), &new_task("Bob's"))
                .unwrap()
                .number,
            1
        );

        assert_eq!(create(&state, "Mine").await, "OPP-2");
        let list = json_of(&state, "/api/projects/test/tasks").await;
        assert_eq!(ids(&list), vec!["OPP-1", "OPP-2"]);
    }
}

#[tokio::test]
async fn a_restart_never_reissues_a_number() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new([local_project(PROJECT, dir.path(), "OPP")]);
    for title in ["Alpha", "Beta"] {
        create(&state, title).await;
    }
    drop(state);

    let restarted = AppState::new([open(PROJECT, dir.path(), op_api::BackendKind::Local)]);
    assert_eq!(create(&restarted, "Gamma").await, "OPP-3");
}

#[tokio::test]
async fn patch_rejects_a_malformed_rank_with_its_reason() {
    let (dir, state) = local_state();
    seed(
        &state,
        &[("tasks/00001-solo.md", &task_file("todo", "Solo", ""))],
    );

    let refused = send(
        &state,
        "PATCH",
        "/api/projects/test/tasks/OPP-1",
        Some(json!({ "rank": "NOT-BASE36" })),
    )
    .await;
    assert_eq!(refused.status(), StatusCode::BAD_REQUEST);
    let message = message_of(&body_json(refused).await);
    assert!(message.contains("rank"), "message: {message}");
    let raw = std::fs::read_to_string(dir.path().join(".plan/tasks/00001-solo.md")).unwrap();
    assert!(!raw.contains("rank"), "a refused rank must not land: {raw}");

    let accepted = send(
        &state,
        "PATCH",
        "/api/projects/test/tasks/OPP-1",
        Some(json!({ "rank": "a5" })),
    )
    .await;
    assert_eq!(accepted.status(), StatusCode::OK);
    assert_eq!(body_json(accepted).await["metadata"]["rank"], "a5");
}

// The web pickers leave out the targets that would close a cycle, but from a snapshot that can be
// stale when the write lands, so the refusal reaches the UI and must carry a reason.
#[tokio::test]
async fn patching_a_parent_that_would_cycle_is_refused_with_its_reason() {
    let (_dir, state) = local_state();
    seed(
        &state,
        &[
            ("tasks/00001-epic.md", &task_file("todo", "Epic", "")),
            (
                "tasks/00002-child.md",
                &task_file("todo", "Child", "parent: ./00001-epic.md\n"),
            ),
        ],
    );

    let refused = send(
        &state,
        "PATCH",
        "/api/projects/test/tasks/OPP-1",
        Some(json!({ "parent": "OPP-2" })),
    )
    .await;
    assert_eq!(refused.status(), StatusCode::BAD_REQUEST);
    let message = message_of(&body_json(refused).await);
    assert!(message.contains("descendant"), "message: {message}");
}

// A patch applies field by field and stops at the first bad key, so a write that reports 400 must
// have changed nothing.
#[tokio::test]
async fn a_patch_refused_for_a_bad_key_writes_nothing() {
    let (dir, state) = local_state();
    let before = task_file("todo", "Alpha", "");
    seed(&state, &[("tasks/00001-alpha.md", &before)]);

    let refused = send(
        &state,
        "PATCH",
        "/api/projects/test/tasks/OPP-1",
        Some(json!({ "status": "done", "parent": "42" })),
    )
    .await;

    assert_eq!(refused.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        std::fs::read_to_string(dir.path().join(".plan/tasks/00001-alpha.md")).unwrap(),
        before,
        "a refused patch must leave the file alone"
    );
    assert_eq!(
        json_of(&state, "/api/projects/test/tasks/OPP-1").await["metadata"]["status"],
        "todo"
    );
}

#[tokio::test]
async fn a_patch_echoes_the_parent_title() {
    let (_dir, state) = local_state();
    seed(
        &state,
        &[
            ("tasks/00001-epic.md", &task_file("todo", "Epic", "")),
            (
                "tasks/00002-child.md",
                &task_file("todo", "Child", "parent: ./00001-epic.md\n"),
            ),
        ],
    );

    let patched = send(
        &state,
        "PATCH",
        "/api/projects/test/tasks/OPP-2",
        Some(json!({ "status": "done" })),
    )
    .await;

    assert_eq!(patched.status(), StatusCode::OK);
    let detail = body_json(patched).await;
    assert_eq!(detail["metadata"]["parent"], "OPP-1");
    assert_eq!(detail["parent_title"], "Epic");
}

#[tokio::test]
async fn the_tree_route_walks_the_whole_subtree() {
    let (_dir, state) = local_state();
    seed(
        &state,
        &[
            ("tasks/00001-root.md", &task_file("todo", "Root", "")),
            (
                "tasks/00002-child.md",
                &task_file("todo", "Child", "parent: ./00001-root.md\n"),
            ),
            (
                "tasks/00003-grandchild.md",
                &task_file("todo", "Grandchild", "parent: ./00002-child.md\n"),
            ),
        ],
    );

    let view = json_of(&state, "/api/projects/test/tasks/OPP-1/tree").await;
    assert_eq!(view["tree"]["id"], "OPP-1");
    assert_eq!(view["tree"]["children"][0]["id"], "OPP-2");
    assert_eq!(view["tree"]["children"][0]["children"][0]["id"], "OPP-3");
    assert!(view.get("cycles").is_none());

    let bounded = json_of(&state, "/api/projects/test/tasks/OPP-1/tree?depth=1").await;
    assert!(
        bounded["tree"]["children"][0]["children"]
            .as_array()
            .unwrap()
            .is_empty(),
        "depth 1 stops at the direct children: {bounded}"
    );
}

// A parent cycle has no bottom to walk to. The subtree stops there, and the response says where, so
// a client does not show a cut hierarchy as a complete one.
#[tokio::test]
async fn the_tree_route_reports_a_truncated_cycle() {
    let (_dir, state) = local_state();
    seed(
        &state,
        &[
            (
                "tasks/00001-a.md",
                &task_file("todo", "A", "parent: ./00002-b.md\n"),
            ),
            (
                "tasks/00002-b.md",
                &task_file("todo", "B", "parent: ./00001-a.md\n"),
            ),
        ],
    );

    let view = json_of(&state, "/api/projects/test/tasks/OPP-1/tree").await;
    assert_eq!(view["cycles"], json!(["OPP-1"]));
}

// The unprefixed task spellings went with the SPA that called them. A caller that still uses one
// must hear so, rather than get the SPA's index.html from the static fallback.
#[tokio::test]
async fn an_api_path_no_route_matches_is_a_404_that_names_itself() {
    let (_dir, state) = local_state();
    for uri in [
        "/api/config",
        "/api/tasks",
        "/api/tasks/OPP-1",
        "/api/projects/test/matrix",
        "/api/projects/test/tasks/OPP-1/branches",
    ] {
        let response = send(&state, "GET", uri, None).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{uri}");
        let message = message_of(&body_json(response).await);
        assert!(message.contains(uri), "{uri} must name itself: {message}");
    }
}

#[tokio::test]
async fn the_unprefixed_board_still_answers() {
    let (_dir, state) = local_state();
    assert_eq!(
        send(&state, "GET", "/api/board", None).await.status(),
        StatusCode::OK
    );
}

#[tokio::test]
async fn an_unknown_project_is_404_that_names_the_registered_ones() {
    let (_dir, state) = local_state();
    for uri in [
        "/api/projects/ghost/tasks",
        "/api/projects/ghost/board",
        "/api/projects/ghost/tasks/OPP-1",
        "/api/projects/ghost/history",
        "/api/projects/ghost/sync",
    ] {
        let response = send(&state, "GET", uri, None).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{uri}");
        let message = message_of(&body_json(response).await);
        assert!(message.contains("ghost"), "{uri}: {message}");
        assert!(message.contains(PROJECT), "{uri}: {message}");
    }
}

// Each project counts its own ids, so one number naming a task in both is correct: the project is
// the coordinate that tells them apart.
#[tokio::test]
async fn two_projects_write_to_their_own_store_under_their_own_numbers() {
    let alpha = tempfile::tempdir().unwrap();
    let beta = tempfile::tempdir().unwrap();
    let state = AppState::new([
        local_project("alpha", alpha.path(), "OPP"),
        local_project("beta", beta.path(), "OPP"),
    ]);

    for (project, dir, title) in [("alpha", &alpha, "Alpha one"), ("beta", &beta, "Beta one")] {
        let id = create_in(&state, project, json!({ "title": title })).await;
        assert_eq!(id, "OPP-1");
        let detail = json_of(&state, &format!("/api/projects/{project}/tasks/OPP-1")).await;
        assert_eq!(detail["title"], title);
        assert_eq!(
            std::fs::read_dir(dir.path().join(".plan/tasks"))
                .unwrap()
                .count(),
            1,
            "{project}: each write lands in its own store only"
        );
    }
}

async fn create_tag(state: &AppState, body: Value) -> axum::response::Response {
    send(state, "POST", "/api/projects/test/tags", Some(body)).await
}

async fn tag_names(state: &AppState) -> Vec<String> {
    json_of(state, "/api/projects/test/tags")
        .await
        .as_array()
        .unwrap()
        .iter()
        .map(|tag| tag["name"].as_str().unwrap().to_owned())
        .collect()
}

fn with_defaults(names: &[&str]) -> Vec<String> {
    let mut all: Vec<String> = DEFAULT_TAGS
        .iter()
        .chain(names)
        .map(|name| (*name).to_owned())
        .collect();
    all.sort();
    all
}

#[tokio::test]
async fn a_new_project_starts_with_the_default_tags() {
    let (_dir, state) = local_state();
    assert_eq!(tag_names(&state).await, with_defaults(&[]));
}

#[tokio::test]
async fn tags_crud_roundtrip() {
    let (_dir, state) = local_state();

    let created = create_tag(
        &state,
        json!({ "name": "Front End", "color": "violet", "description": "The web client." }),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let view = body_json(created).await;
    assert_eq!(view["name"], "front-end", "the name is normalized");
    assert_eq!(view["display"], "Front End", "the heading keeps the case");
    assert_eq!(view["color"], "violet");
    assert_eq!(view["description"], "The web client.");

    assert_eq!(tag_names(&state).await, with_defaults(&["front-end"]));
    assert_eq!(
        json_of(&state, "/api/projects/test/tags/front-end").await,
        view
    );

    let patched = send(
        &state,
        "PATCH",
        "/api/projects/test/tags/front-end",
        Some(json!({ "color": "teal", "description": "The SPA." })),
    )
    .await;
    assert_eq!(patched.status(), StatusCode::OK);
    let view = body_json(patched).await;
    assert_eq!(view["color"], "teal");
    assert_eq!(view["description"], "The SPA.");

    let deleted = send(&state, "DELETE", "/api/projects/test/tags/front-end", None).await;
    assert_eq!(deleted.status(), StatusCode::NO_CONTENT);
    assert_eq!(tag_names(&state).await, with_defaults(&[]));
}

#[tokio::test]
async fn a_tag_without_a_color_is_given_one() {
    let (_dir, state) = local_state();

    let created = create_tag(&state, json!({ "name": "backend" })).await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let view = body_json(created).await;
    assert!(
        view["color"].is_string(),
        "an omitted color is derived from the name, not left unset"
    );
    assert!(
        view.get("description").is_none(),
        "a tag with no prose carries no description"
    );
}

#[tokio::test]
async fn creating_a_registered_tag_again_is_a_conflict() {
    let (_dir, state) = local_state();

    assert_eq!(
        create_tag(&state, json!({ "name": "backend" }))
            .await
            .status(),
        StatusCode::CREATED
    );
    let again = create_tag(&state, json!({ "name": "Backend" })).await;
    assert_eq!(
        again.status(),
        StatusCode::CONFLICT,
        "both spellings normalize to one name"
    );
}

// The palette is a closed enum, so a name outside it fails to deserialize: the same 422 every other
// closed field answers with.
#[tokio::test]
async fn a_color_outside_the_palette_is_refused() {
    let (_dir, state) = local_state();
    let created = create_tag(&state, json!({ "name": "backend", "color": "chartreuse" })).await;
    assert_eq!(created.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn a_name_that_cannot_be_normalized_is_refused() {
    let (_dir, state) = local_state();

    let created = create_tag(&state, json!({ "name": "C++" })).await;
    assert_eq!(created.status(), StatusCode::BAD_REQUEST);
    let message = message_of(&body_json(created).await);
    assert!(
        message.contains("lowercase letters"),
        "the refusal names the rule: {message}"
    );
}

#[tokio::test]
async fn an_unregistered_tag_is_not_found() {
    let (_dir, state) = local_state();
    let got = send(&state, "GET", "/api/projects/test/tags/backend", None).await;
    assert_eq!(got.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn deleting_a_referenced_tag_needs_force() {
    let (_dir, state) = local_state();
    assert_eq!(
        create_tag(&state, json!({ "name": "backend" }))
            .await
            .status(),
        StatusCode::CREATED
    );
    create_in(
        &state,
        PROJECT,
        json!({ "title": "Wire the parser", "tags": ["backend"] }),
    )
    .await;

    let refused = send(&state, "DELETE", "/api/projects/test/tags/backend", None).await;
    assert_eq!(refused.status(), StatusCode::CONFLICT);
    // The status covers several refusals and `force` answers only this one, so the caller reads the
    // field rather than the sentence to decide whether to offer it.
    let body = body_json(refused).await;
    assert_eq!(body["reason"], "tag_referenced");
    assert!(
        !message_of(&body).contains("--force"),
        "the remedy is the caller's own spelling: {body}"
    );
    assert_eq!(tag_names(&state).await, with_defaults(&["backend"]));

    let forced = send(
        &state,
        "DELETE",
        "/api/projects/test/tags/backend?force=true",
        None,
    )
    .await;
    assert_eq!(forced.status(), StatusCode::NO_CONTENT);
    assert_eq!(tag_names(&state).await, with_defaults(&[]));
}

#[tokio::test]
async fn renaming_a_tag_rewrites_the_tasks_that_reference_it() {
    let (_dir, state) = local_state();
    assert_eq!(
        create_tag(&state, json!({ "name": "backend" }))
            .await
            .status(),
        StatusCode::CREATED
    );
    let id = create_in(
        &state,
        PROJECT,
        json!({ "title": "Wire the parser", "tags": ["backend"] }),
    )
    .await;

    let renamed = send(
        &state,
        "PATCH",
        "/api/projects/test/tags/backend",
        Some(json!({ "name": "Infra" })),
    )
    .await;
    assert_eq!(renamed.status(), StatusCode::OK);
    let view = body_json(renamed).await;
    assert_eq!(view["name"], "infra");
    assert_eq!(view["display"], "Infra");
    assert_eq!(tag_names(&state).await, with_defaults(&["infra"]));

    let task = json_of(&state, &format!("/api/projects/test/tasks/{id}")).await;
    assert_eq!(
        task["metadata"]["tags"],
        json!(["infra"]),
        "the reference moved with the tag"
    );
}

#[tokio::test]
async fn a_rename_keeps_the_color_and_the_description_it_does_not_name() {
    let (_dir, state) = local_state();
    assert_eq!(
        create_tag(
            &state,
            json!({ "name": "backend", "color": "amber", "description": "Behind the API." })
        )
        .await
        .status(),
        StatusCode::CREATED
    );

    let renamed = send(
        &state,
        "PATCH",
        "/api/projects/test/tags/backend",
        Some(json!({ "name": "Infra" })),
    )
    .await;
    assert_eq!(renamed.status(), StatusCode::OK);
    let view = body_json(renamed).await;
    assert_eq!(view["name"], "infra");
    assert_eq!(view["color"], "amber");
    assert_eq!(view["description"], "Behind the API.");
}

#[tokio::test]
async fn a_rename_that_only_changes_the_case_moves_the_heading_alone() {
    let (_dir, state) = local_state();
    assert_eq!(
        create_tag(&state, json!({ "name": "backend" }))
            .await
            .status(),
        StatusCode::CREATED
    );

    let renamed = send(
        &state,
        "PATCH",
        "/api/projects/test/tags/backend",
        Some(json!({ "name": "Backend" })),
    )
    .await;
    assert_eq!(renamed.status(), StatusCode::OK);
    let view = body_json(renamed).await;
    assert_eq!(view["name"], "backend");
    assert_eq!(view["display"], "Backend");
    assert_eq!(tag_names(&state).await, with_defaults(&["backend"]));
}

#[tokio::test]
async fn a_patch_can_clear_a_description() {
    let (_dir, state) = local_state();
    assert_eq!(
        create_tag(
            &state,
            json!({ "name": "backend", "description": "Behind the API." })
        )
        .await
        .status(),
        StatusCode::CREATED
    );

    let patched = send(
        &state,
        "PATCH",
        "/api/projects/test/tags/backend",
        Some(json!({ "description": null })),
    )
    .await;
    assert_eq!(patched.status(), StatusCode::OK);
    let view = body_json(patched).await;
    assert!(
        view.get("description").is_none(),
        "a cleared description leaves no prose behind: {view}"
    );
    assert_eq!(view["display"], "backend", "the heading stays");
}

#[tokio::test]
async fn renaming_onto_a_registered_name_is_a_conflict() {
    let (_dir, state) = local_state();
    for name in ["backend", "infra"] {
        assert_eq!(
            create_tag(&state, json!({ "name": name })).await.status(),
            StatusCode::CREATED
        );
    }

    let renamed = send(
        &state,
        "PATCH",
        "/api/projects/test/tags/backend",
        Some(json!({ "name": "infra" })),
    )
    .await;
    assert_eq!(renamed.status(), StatusCode::CONFLICT);
    assert_eq!(
        tag_names(&state).await,
        with_defaults(&["backend", "infra"]),
        "a refused rename leaves both tags alone"
    );
}

#[tokio::test]
async fn a_task_can_only_carry_registered_tags() {
    let (_dir, state) = local_state();

    let refused = send(
        &state,
        "POST",
        "/api/projects/test/tasks",
        Some(json!({ "title": "Wire the parser", "tags": ["backend"] })),
    )
    .await;
    assert_eq!(refused.status(), StatusCode::BAD_REQUEST);
    let body = body_json(refused).await;
    assert_eq!(body["reason"], "tag_unregistered");
    let message = message_of(&body);
    assert!(
        message.contains("backend"),
        "the refusal names the tag: {message}"
    );
    assert!(
        !message.contains("openplan"),
        "the remedy is the caller's own spelling: {message}"
    );

    assert_eq!(
        create_tag(&state, json!({ "name": "backend" }))
            .await
            .status(),
        StatusCode::CREATED
    );
    let id = create_in(
        &state,
        PROJECT,
        json!({ "title": "Wire the parser", "tags": ["backend"] }),
    )
    .await;

    let patched = send(
        &state,
        "PATCH",
        &format!("/api/projects/test/tasks/{id}"),
        Some(json!({ "tags": ["backend", "wip"] })),
    )
    .await;
    assert_eq!(
        patched.status(),
        StatusCode::BAD_REQUEST,
        "the whole set is validated, not only what the patch adds"
    );

    let cleared = send(
        &state,
        "PATCH",
        &format!("/api/projects/test/tasks/{id}"),
        Some(json!({ "tags": [] })),
    )
    .await;
    assert_eq!(cleared.status(), StatusCode::OK);
    assert_eq!(body_json(cleared).await["metadata"]["tags"], json!([]));
}

async fn comment(state: &AppState, id: &str, body: Value) -> axum::response::Response {
    send(
        state,
        "POST",
        &format!("/api/projects/test/tasks/{id}/comments"),
        Some(body),
    )
    .await
}

#[tokio::test]
async fn comments_append_and_read_back() {
    let (_dir, state) = local_state();
    let id = create(&state, "Ship login").await;

    let response = comment(
        &state,
        &id,
        json!({ "text": "hello", "author": "Milan Suk", "agent": "claude-code" }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let written = body_json(response).await;
    assert_eq!(written["author"], "Milan Suk");
    assert_eq!(written["agent"], "claude-code");
    assert_eq!(written["text"], "hello");

    let read = json_of(&state, &format!("/api/projects/test/tasks/{id}/comments")).await;
    assert_eq!(read.as_array().unwrap().len(), 1);
    assert_eq!(read[0]["text"], "hello");
    assert_eq!(read[0]["at"], written["at"]);
}

#[tokio::test]
async fn a_comment_never_reaches_the_detail_body() {
    let (_dir, state) = local_state();
    let id = create(&state, "Ship login").await;
    comment(
        &state,
        &id,
        json!({ "text": "hello", "author": "Milan Suk" }),
    )
    .await;

    let detail = json_of(&state, &format!("/api/projects/test/tasks/{id}")).await;

    assert_eq!(detail["description"], "", "the description carries no log");
    assert_eq!(detail["comments"][0]["text"], "hello");
    assert_eq!(detail["comments"][0]["agent"], Value::Null);
}

#[tokio::test]
async fn a_patch_keeps_the_comment_log_out_of_the_body_it_echoes() {
    let (_dir, state) = local_state();
    let id = create(&state, "Ship login").await;
    comment(
        &state,
        &id,
        json!({ "text": "hello", "author": "Milan Suk" }),
    )
    .await;

    let echoed = body_json(
        send(
            &state,
            "PATCH",
            &format!("/api/projects/test/tasks/{id}"),
            Some(json!({ "status": "in_progress" })),
        )
        .await,
    )
    .await;

    assert_eq!(echoed["description"], "");
    assert_eq!(echoed["comments"][0]["text"], "hello");
}

#[tokio::test]
async fn a_list_row_counts_the_comments() {
    let (_dir, state) = local_state();
    let id = create(&state, "Ship login").await;
    for text in ["one", "two"] {
        comment(&state, &id, json!({ "text": text, "author": "Milan Suk" })).await;
    }

    let items = json_of(&state, "/api/projects/test/tasks").await;
    assert_eq!(items[0]["comment_count"], 2);
}

#[tokio::test]
async fn an_empty_or_unsigned_comment_is_refused() {
    let (_dir, state) = local_state();
    let id = create(&state, "Ship login").await;

    let empty = comment(
        &state,
        &id,
        json!({ "text": "  \n ", "author": "Milan Suk" }),
    )
    .await;
    assert_eq!(empty.status(), StatusCode::BAD_REQUEST);

    let unsigned = comment(&state, &id, json!({ "text": "a", "author": "" })).await;
    assert_eq!(unsigned.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_comment_on_a_missing_task_is_404() {
    let (_dir, state) = local_state();
    let response = comment(&state, "OPP-9", json!({ "text": "a", "author": "Ada" })).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_search_matches_comment_text() {
    let (_dir, state) = local_state();
    let id = create(&state, "Ship login").await;
    comment(
        &state,
        &id,
        json!({ "text": "the parser mishandles a tab", "author": "Milan Suk" }),
    )
    .await;

    let hits = json_of(&state, "/api/projects/test/search?q=mishandles&fresh=true").await;
    assert_eq!(hits.as_array().unwrap().len(), 1, "{hits}");
    assert_eq!(hits[0]["task"]["id"], id);
}

// An entry heading is one line, and a line break in it would let one write append entries nobody
// signed.
#[tokio::test]
async fn a_line_break_in_the_identity_is_refused() {
    let (_dir, state) = local_state();
    let id = create(&state, "Ship login").await;

    let forged = comment(
        &state,
        &id,
        json!({
            "text": "real",
            "author": "Evil",
            "agent": "x\n\n### 2026-01-02T00:00:00Z by Forged\n\n> forged entry",
        }),
    )
    .await;
    assert_eq!(forged.status(), StatusCode::BAD_REQUEST);
    assert!(message_of(&body_json(forged).await).contains("line break"));

    let signed = comment(
        &state,
        &id,
        json!({ "text": "real", "author": "Ada\nBogus" }),
    )
    .await;
    assert_eq!(signed.status(), StatusCode::BAD_REQUEST);

    let read = json_of(&state, &format!("/api/projects/test/tasks/{id}/comments")).await;
    assert_eq!(read.as_array().unwrap().len(), 0, "{read}");
}

#[tokio::test]
async fn openapi_spec_is_served_over_http() {
    let response = get("/api-docs/openapi.json").await;
    assert_eq!(response.status(), StatusCode::OK);
    let spec = body_json(response).await;
    assert_eq!(spec["info"]["title"], "openplan");
    assert!(spec["paths"].get("/api/projects/{project}/tasks").is_some());
    assert!(
        spec["paths"]
            .get("/api/projects/{project}/tasks/{id}")
            .is_some()
    );
}

#[tokio::test]
async fn swagger_ui_page_is_served() {
    let response = get("/swagger-ui/").await;
    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    assert!(content_type.starts_with("text/html"));
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert!(
        String::from_utf8_lossy(&bytes)
            .to_lowercase()
            .contains("swagger")
    );
}

#[test]
fn the_openapi_spec_documents_every_json_api_route() {
    let spec = serde_json::to_value(op_server::openapi()).unwrap();
    let paths = spec["paths"].as_object().unwrap();
    for (route, methods) in [
        ("/health", &["get"][..]),
        ("/api/projects", &["get", "post"]),
        ("/api/projects/{project}", &["patch", "delete"]),
        ("/api/projects/{project}/tasks", &["get", "post"]),
        (
            "/api/projects/{project}/tasks/{id}",
            &["get", "patch", "delete"],
        ),
        ("/api/projects/{project}/tasks/{id}/file", &["put"]),
        ("/api/projects/{project}/tasks/{id}/tree", &["get"]),
        (
            "/api/projects/{project}/tasks/{id}/comments",
            &["get", "post"],
        ),
        ("/api/projects/{project}/tasks/{id}/history", &["get"]),
        (
            "/api/projects/{project}/tasks/{id}/revisions/{revision}",
            &["get"],
        ),
        ("/api/projects/{project}/history", &["get"]),
        ("/api/projects/{project}/sync", &["get", "post"]),
        ("/api/projects/{project}/board", &["get"]),
        ("/api/projects/{project}/search", &["get"]),
        ("/api/projects/{project}/tags", &["get", "post"]),
        (
            "/api/projects/{project}/tags/{name}",
            &["get", "patch", "delete"],
        ),
        ("/api/board", &["get"]),
        ("/api/search", &["get"]),
        ("/api/flow/drawing", &["get"]),
        ("/api/diagram", &["post"]),
    ] {
        for method in methods {
            assert!(
                paths
                    .get(route)
                    .is_some_and(|path| path.get(*method).is_some()),
                "{method} {route} missing from the spec"
            );
        }
    }
    for schema in [
        "Board",
        "Drawing",
        "DiagramSource",
        "HistoryEntry",
        "TaskAtRevision",
        "SyncView",
        "SyncResult",
        "WriteTaskFile",
        "RegisterProject",
    ] {
        assert!(
            spec["components"]["schemas"].get(schema).is_some(),
            "{schema} must reach the spec the web client is generated from"
        );
    }
    for gone in [
        "/api/projects/{project}/matrix",
        "/api/projects/{project}/tasks/{id}/branches",
    ] {
        assert!(
            !paths.contains_key(gone),
            "{gone} is gone with the branches"
        );
    }
}

// The generated web client turns each documented refusal into a typed error that carries the
// server's reason. An undocumented status reaches the UI as a bare status code.
#[test]
fn the_openapi_spec_documents_every_refusal_with_its_reason() {
    let spec = serde_json::to_value(op_server::openapi()).unwrap();
    for (method, route, status) in [
        ("post", "/api/projects", "400"),
        ("post", "/api/projects", "409"),
        ("post", "/api/projects", "503"),
        ("get", "/api/projects/{project}/tasks", "404"),
        ("post", "/api/projects/{project}/tasks", "400"),
        ("post", "/api/projects/{project}/tasks", "404"),
        ("post", "/api/projects/{project}/tasks", "409"),
        ("get", "/api/flow/drawing", "400"),
        ("get", "/api/flow/drawing", "404"),
        ("get", "/api/flow/drawing", "422"),
        ("get", "/api/flow/drawing", "503"),
        ("post", "/api/diagram", "422"),
        ("get", "/api/projects/{project}/board", "404"),
        ("get", "/api/projects/{project}/tasks/{id}", "400"),
        ("get", "/api/projects/{project}/tasks/{id}", "404"),
        ("patch", "/api/projects/{project}/tasks/{id}", "400"),
        ("patch", "/api/projects/{project}/tasks/{id}", "404"),
        ("patch", "/api/projects/{project}/tasks/{id}", "409"),
        ("delete", "/api/projects/{project}/tasks/{id}", "400"),
        ("delete", "/api/projects/{project}/tasks/{id}", "404"),
        ("put", "/api/projects/{project}/tasks/{id}/file", "400"),
        ("put", "/api/projects/{project}/tasks/{id}/file", "404"),
        ("put", "/api/projects/{project}/tasks/{id}/file", "409"),
        ("get", "/api/projects/{project}/history", "404"),
        ("get", "/api/projects/{project}/tasks/{id}/history", "400"),
        (
            "get",
            "/api/projects/{project}/tasks/{id}/revisions/{revision}",
            "404",
        ),
        ("get", "/api/projects/{project}/sync", "404"),
        ("post", "/api/projects/{project}/sync", "404"),
        ("post", "/api/projects/{project}/sync", "502"),
        ("delete", "/api/projects/{project}/tags/{name}", "409"),
    ] {
        let responses = &spec["paths"][route][method]["responses"];
        assert!(
            responses.get(status).is_some(),
            "{method} {route} must document {status}"
        );
    }
    for (route, methods) in spec["paths"].as_object().unwrap() {
        for (method, operation) in methods.as_object().unwrap() {
            for (status, response) in operation["responses"].as_object().unwrap() {
                if status.starts_with('4') || status.starts_with('5') {
                    assert_eq!(
                        response["content"]["application/json"]["schema"]["$ref"],
                        "#/components/schemas/ApiErrorBody",
                        "{method} {route} -> {status} must document its reason body"
                    );
                }
            }
        }
    }
}

// The merged board carries no project segment, so every row names its own, and a client links a
// task from the row alone.
#[test]
fn the_spec_documents_the_merged_board_and_the_project_every_task_names() {
    let spec = serde_json::to_value(op_server::openapi()).unwrap();
    assert_eq!(
        spec["paths"]["/api/board"]["get"]["responses"]["200"]["content"]["application/json"]["schema"]
            ["$ref"],
        "#/components/schemas/Board"
    );
    assert!(
        spec["paths"]["/api/board"]["get"]["parameters"].is_null(),
        "the merged board takes no project"
    );
    let schemas = &spec["components"]["schemas"];
    for schema in ["TaskListItem", "TaskDetail"] {
        assert_eq!(
            schemas[schema]["properties"]["project"]["type"], "string",
            "{schema}.project must be a plain string"
        );
        assert!(
            schemas[schema]["required"]
                .as_array()
                .unwrap()
                .iter()
                .any(|field| field == "project"),
            "{schema}.project must always be present"
        );
        for gone in ["headline", "branches", "write_target"] {
            assert!(
                schemas[schema]["properties"].get(gone).is_none(),
                "{schema}.{gone} is gone with the branches"
            );
        }
    }
}

// A field the server skips when empty is absent, never null, so the spec must not widen it to
// nullable: that would push an impossible `| null` into every generated client type.
#[test]
fn optional_response_fields_are_absent_rather_than_nullable() {
    let spec = serde_json::to_value(op_server::openapi()).unwrap();
    let schemas = &spec["components"]["schemas"];
    for (schema, field) in [
        ("TaskChild", "rank"),
        ("BoardRow", "parent_title"),
        ("TaskDetail", "parent_title"),
        ("RevisionView", "email"),
        ("RevisionView", "agent"),
        ("DocumentChange", "task"),
        ("DocumentChange", "tag"),
        ("DocumentChange", "doc"),
        ("DocChange", "renamed_from"),
        ("SyncView", "error"),
        ("ProjectView", "git_common_dir"),
    ] {
        assert_eq!(
            schemas[schema]["properties"][field]["type"], "string",
            "{schema}.{field} must be a plain optional string"
        );
    }
    for (schema, field) in [("TaskAtRevision", "task"), ("ProjectView", "sync")] {
        let property = &schemas[schema]["properties"][field];
        assert!(
            property.get("$ref").is_some()
                || property["allOf"]
                    .as_array()
                    .is_some_and(|all| all.len() == 1),
            "{schema}.{field} must be a plain optional reference: {property}"
        );
        assert!(
            !property.to_string().contains("null"),
            "{schema}.{field}: {property}"
        );
    }
}
