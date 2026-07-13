use std::net::SocketAddr;
use std::thread::JoinHandle;

use op_api::DaemonInfo;
use op_client::Client;
use op_server::AppState;

fn spawn_server(info: DaemonInfo) -> (SocketAddr, JoinHandle<()>) {
    let (tx, rx) = std::sync::mpsc::channel();
    let handle = std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            tx.send(listener.local_addr().unwrap()).unwrap();
            let state = AppState::default().with_health(info);
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
