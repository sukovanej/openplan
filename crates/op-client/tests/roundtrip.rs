use std::net::SocketAddr;
use std::thread::JoinHandle;

use op_api::DaemonInfo;
use op_client::Client;
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

#[test]
fn health_reads_identity_then_shutdown_stops_the_server() {
    let info = DaemonInfo {
        pid: 4242,
        port: 7,
        version: "1.2.3".to_owned(),
        started_at: 99,
    };
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
