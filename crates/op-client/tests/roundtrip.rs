use std::net::SocketAddr;
use std::path::PathBuf;
use std::thread::JoinHandle;

use op_api::{
    BackendKind, CreateComment, CreateTag, CreateTask, DaemonInfo, Status, TagPatch, TaskPatch,
};
use op_client::{Client, ClientError, Identity};
use op_server::{AppState, Location, Project, REGISTRY_FILE};

struct Daemon {
    base: String,
    handle: Option<JoinHandle<()>>,
    home: tempfile::TempDir,
}

impl Daemon {
    // The daemon runs on its own runtime, as it does in production, so the blocking client can call
    // it from the test thread.
    fn spawn(state: AppState) -> Self {
        let home = tempfile::tempdir().unwrap();
        let state = state.with_registry(home.path().join(REGISTRY_FILE));
        let (tx, rx) = std::sync::mpsc::channel::<SocketAddr>();
        let handle = std::thread::spawn(move || {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            runtime.block_on(async move {
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                tx.send(listener.local_addr().unwrap()).unwrap();
                op_server::serve(listener, state, std::future::pending::<()>())
                    .await
                    .unwrap();
            });
        });
        Self {
            base: format!("http://{}", rx.recv().unwrap()),
            handle: Some(handle),
            home,
        }
    }

    fn with_one_project(info: DaemonInfo) -> (Self, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let location = Location::find(dir.path(), Some(BackendKind::Local)).unwrap();
        let project = Project::open("test", location).unwrap();
        project
            .tracker()
            .init(project.machine(), "OPP".parse().unwrap())
            .unwrap();
        project.reload();
        (Self::spawn(AppState::new([project]).with_health(info)), dir)
    }

    fn stop(mut self) {
        assert!(Client::default().shutdown(&self.base));
        if let Some(handle) = self.handle.take() {
            handle.join().unwrap();
        }
    }
}

fn info() -> DaemonInfo {
    DaemonInfo {
        pid: 4242,
        port: 7,
        version: "1.2.3".to_owned(),
        started_at: 99,
    }
}

fn new_task(title: &str) -> CreateTask {
    CreateTask {
        title: title.to_owned(),
        status: None,
        parent: None,
        dependencies: Vec::new(),
        tags: Vec::new(),
        body: None,
    }
}

#[test]
fn health_reads_identity_then_shutdown_stops_the_server() {
    let (daemon, _dir) = Daemon::with_one_project(info());
    let base = daemon.base.clone();
    let client = Client::default();

    assert_eq!(client.health(&base), Some(info()));

    daemon.stop();
    assert!(
        client.health(&base).is_none(),
        "health must fail once the server has stopped"
    );
}

#[test]
fn health_is_none_when_nothing_listens() {
    let client = Client::default();
    assert!(client.health("http://127.0.0.1:1").is_none());
}

#[test]
fn crud_roundtrips_through_the_daemon() {
    let (daemon, _dir) = Daemon::with_one_project(info());
    let base = &daemon.base;
    let client = Client::default();

    let id = client
        .create_task(base, "test", &new_task("Ship login"))
        .unwrap();
    assert_eq!(id, "OPP-1", "the daemon answers in the key spelling");

    let patched = client
        .patch_task(
            base,
            "test",
            &id,
            &TaskPatch {
                status: Some(Status::InProgress),
                ..TaskPatch::default()
            },
        )
        .unwrap();
    assert_eq!(patched.metadata.status(), Some(Status::InProgress));
    assert_eq!(patched.title, "Ship login");
    assert_eq!(client.task(base, "test", &id).unwrap(), patched);
    assert_eq!(client.tasks(base, "test").unwrap().len(), 1);
    assert_eq!(client.search(base, "test", "login").unwrap().len(), 1);
    assert_eq!(
        client.task_tree(base, "test", &id, None).unwrap().tree.id,
        id
    );

    client.delete_task(base, "test", &id).unwrap();
    let gone = client
        .patch_task(base, "test", &id, &TaskPatch::default())
        .expect_err("the task is deleted");
    assert!(
        matches!(gone, ClientError::Refused { status: 404, .. }),
        "{gone:?}"
    );
    assert!(
        gone.to_string().contains(&id),
        "the daemon's own reason reaches the caller: {gone}"
    );

    daemon.stop();
}

#[test]
fn a_whole_file_and_the_comments_roundtrip() {
    let (daemon, _dir) = Daemon::with_one_project(info());
    let base = &daemon.base;
    let client = Client::default();
    let id = client
        .create_task(base, "test", &new_task("Draft"))
        .unwrap();

    let comment = client
        .add_comment(
            base,
            "test",
            &id,
            &CreateComment {
                text: "hello".to_owned(),
                author: "Ada".to_owned(),
                agent: None,
            },
        )
        .unwrap();
    assert_eq!(client.comments(base, "test", &id).unwrap(), vec![comment]);

    let newest = client
        .task_history(base, "test", &id, None, Some(1))
        .unwrap();
    let revision = &newest[0].revision.id;
    let raw = client
        .task_revision(base, "test", &id, revision)
        .unwrap()
        .task
        .expect("the task exists at its newest revision")
        .raw;
    let edited = raw.replace("# Draft", "# Final");
    let written = client.write_task_file(base, "test", &id, &edited).unwrap();
    assert_eq!(written.title, "Final");
    assert_eq!(written.comments.len(), 1);

    let (head, _) = edited.split_once("## Comments").unwrap();
    let refused = client
        .write_task_file(base, "test", &id, head)
        .expect_err("the comment log is append-only");
    assert!(
        matches!(refused, ClientError::Refused { status: 400, .. }),
        "{refused:?}"
    );

    daemon.stop();
}

#[test]
fn the_identity_of_the_client_signs_its_writes() {
    let (daemon, _dir) = Daemon::with_one_project(info());
    let base = &daemon.base;
    let client = Client::default().with_identity(Identity {
        name: Some("Zoë Ada".to_owned()),
        email: Some("zoe@example.com".to_owned()),
        agent: Some("claude-code".to_owned()),
    });

    let id = client
        .create_task(base, "test", &new_task("Signed"))
        .unwrap();
    let history = client.history(base, "test", None, None).unwrap();
    let revision = &history[0].revision;
    assert_eq!(revision.author, "Zoë Ada");
    assert_eq!(revision.email.as_deref(), Some("zoe@example.com"));
    assert_eq!(revision.agent.as_deref(), Some("claude-code"));
    assert_eq!(history[0].changes[0].task.as_deref(), Some(id.as_str()));

    let older = client
        .history(base, "test", Some(&revision.id), Some(10))
        .unwrap();
    assert_eq!(older.len(), history.len() - 1);

    daemon.stop();
}

#[test]
fn tags_roundtrip_and_a_referenced_tag_names_its_refusal() {
    let (daemon, _dir) = Daemon::with_one_project(info());
    let base = &daemon.base;
    let client = Client::default();

    let tag = client
        .create_tag(
            base,
            "test",
            &CreateTag {
                name: "backend".to_owned(),
                color: None,
                description: None,
            },
        )
        .unwrap();
    assert_eq!(client.tag(base, "test", "backend").unwrap(), tag);
    assert!(client.tags(base, "test").unwrap().contains(&tag));

    let renamed = client
        .patch_tag(
            base,
            "test",
            "backend",
            &TagPatch {
                name: Some("Infra".to_owned()),
                ..TagPatch::default()
            },
        )
        .unwrap();
    assert_eq!(renamed.name, "infra");

    client
        .create_task(
            base,
            "test",
            &CreateTask {
                tags: vec!["infra".to_owned()],
                ..new_task("Tagged")
            },
        )
        .unwrap();
    let refused = client
        .delete_tag(base, "test", "infra", false)
        .expect_err("a task carries the tag");
    assert!(
        matches!(
            refused,
            ClientError::Refused {
                status: 409,
                reason: Some(op_api::Refusal::TagReferenced),
                ..
            }
        ),
        "{refused:?}"
    );
    client.delete_tag(base, "test", "infra", true).unwrap();

    daemon.stop();
}

#[test]
fn a_local_project_has_no_sync() {
    let (daemon, _dir) = Daemon::with_one_project(info());
    let client = Client::default();

    for refused in [
        client.sync_status(&daemon.base, "test").map(drop),
        client.sync(&daemon.base, "test").map(drop),
    ] {
        assert!(
            matches!(refused, Err(ClientError::Refused { status: 404, .. })),
            "{refused:?}"
        );
    }

    daemon.stop();
}

#[test]
fn projects_register_rename_and_leave_through_the_daemon() {
    let daemon = Daemon::spawn(AppState::new([]));
    let base = &daemon.base;
    let client = Client::default();
    let dir = tempfile::tempdir().unwrap();

    let (view, created) = client
        .register_project(base, dir.path(), Some(BackendKind::Local), Some("OPP"))
        .unwrap();
    assert!(created);
    assert_eq!(view.backend, BackendKind::Local);
    assert_eq!(view.abbreviation, "OPP");
    assert_eq!(
        PathBuf::from(&view.root),
        dir.path().canonicalize().unwrap()
    );

    let (again, created) = client
        .register_project(base, dir.path(), None, None)
        .unwrap();
    assert!(!created);
    assert_eq!(again.name, view.name);

    let refused = client
        .register_project(base, dir.path(), None, Some("ZZZ"))
        .expect_err("the project already uses OPP");
    assert!(
        matches!(refused, ClientError::Refused { status: 409, .. }),
        "{refused:?}"
    );

    assert_eq!(
        client
            .create_task(base, &view.name, &new_task("First"))
            .unwrap(),
        "OPP-1"
    );
    let renamed = client.rename_project(base, &view.name, "work").unwrap();
    assert_eq!(renamed.name, "work");
    let listed = client.projects(base, op_client::READ_TIMEOUT).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].name, "work");
    assert!(daemon.home.path().join(REGISTRY_FILE).exists());

    client.remove_project(base, "work").unwrap();
    assert!(
        client
            .projects(base, op_client::READ_TIMEOUT)
            .unwrap()
            .is_empty()
    );
    assert!(dir.path().join(".plan/tasks/00001-first.md").exists());

    daemon.stop();
}

#[test]
fn a_route_that_answers_with_the_page_is_not_json() {
    let (daemon, _dir) = Daemon::with_one_project(info());
    let client = Client::default();

    let refused = client
        .tasks(&format!("{}/not-the-api", daemon.base), "test")
        .expect_err("the page is not a task list");
    assert!(
        matches!(refused, ClientError::NotJson { .. }),
        "{refused:?}"
    );

    daemon.stop();
}
