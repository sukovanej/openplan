use std::net::SocketAddr;
use std::thread::JoinHandle;

use op_api::{CreateTask, DaemonInfo, Status, TaskPatch};
use op_client::{Client, ClientError};
use op_server::AppState;

fn git(dir: &std::path::Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .current_dir(dir)
        .args(args)
        .status()
        .expect("git must be installed for this test");
    assert!(status.success(), "git {args:?} failed");
}

fn spawn_server(info: DaemonInfo) -> (SocketAddr, JoinHandle<()>) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    git(&root, &["init", "-q", "-b", "main"]);
    std::fs::create_dir_all(root.join(".plan/tasks")).unwrap();
    let store = op_store::Store::open(&root).unwrap();
    let repo = op_git::Repo::discover(&root).unwrap();

    let (tx, rx) = std::sync::mpsc::channel();
    let handle = std::thread::spawn(move || {
        // Keep the temp repo alive for the server's lifetime.
        let _dir = dir;
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            tx.send(listener.local_addr().unwrap()).unwrap();
            let state = AppState::new(repo, store).with_health(info);
            op_server::serve(listener, state, std::future::pending::<()>())
                .await
                .unwrap();
        });
    });
    (rx.recv().unwrap(), handle)
}

fn info() -> DaemonInfo {
    DaemonInfo {
        pid: 4242,
        port: 7,
        version: "1.2.3".to_owned(),
        started_at: 99,
        repo: Some("/tmp/repo/.git".to_owned()),
    }
}

#[test]
fn health_reads_identity_then_shutdown_stops_the_server() {
    let info = info();
    let (addr, handle) = spawn_server(info.clone());
    let base = format!("http://{addr}");
    let client = Client::default();

    assert_eq!(client.health(&base), Some(info));

    assert!(client.shutdown(&base), "admin shutdown must return success");
    handle.join().unwrap();

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

fn new_task(title: &str) -> CreateTask {
    CreateTask {
        title: title.to_owned(),
        status: None,
        parent: None,
        deps: Vec::new(),
        body: None,
    }
}

#[test]
fn crud_roundtrips_through_the_daemon() {
    let (addr, handle) = spawn_server(info());
    let base = format!("http://{addr}");
    let client = Client::default();

    let id = client
        .create_task(&base, "main", &new_task("Ship login"))
        .unwrap();
    assert!(id.starts_with("ship-login-"), "slug id: {id}");

    let patched = client
        .patch_task(
            &base,
            "main",
            &id,
            &TaskPatch {
                status: Some(Status::InProgress),
                ..TaskPatch::default()
            },
        )
        .unwrap();
    assert_eq!(patched.metadata.status(), Some(Status::InProgress));
    assert_eq!(patched.title, "Ship login");

    client.delete_task(&base, "main", &id).unwrap();
    let gone = client
        .patch_task(&base, "main", &id, &TaskPatch::default())
        .expect_err("the task is deleted");
    assert!(
        matches!(gone, ClientError::Refused { status: 404, .. }),
        "{gone:?}"
    );
    assert!(
        gone.to_string().contains(&id),
        "the daemon's own reason reaches the caller: {gone}"
    );

    client.shutdown(&base);
    handle.join().unwrap();
}

#[test]
fn a_write_to_a_branch_with_no_live_worktree_is_refused() {
    let (addr, handle) = spawn_server(info());
    let base = format!("http://{addr}");
    let client = Client::default();

    // The branch a write names is the branch it lands on, or nothing: no worktree holds `ghost`, so
    // the daemon refuses instead of writing to whatever it happens to have checked out.
    let refused = client
        .create_task(&base, "ghost", &new_task("Ship login"))
        .expect_err("no live worktree for ghost");
    assert!(
        matches!(refused, ClientError::Refused { status: 409, .. }),
        "{refused:?}"
    );

    client.shutdown(&base);
    handle.join().unwrap();
}
