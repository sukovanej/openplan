use std::net::SocketAddr;
use std::path::Path;

use anyhow::{Context, Result, bail};
use fs2::FileExt as _;
use op_server::AppState;
use op_store::Store;
use op_watch::Watcher;
use tokio::signal::unix::{SignalKind, signal};
use tracing_subscriber::EnvFilter;

use crate::daemon::{DaemonInfo, Home, now_unix};

pub async fn run(home: Home, port: u16, root: &Path) -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .try_init()
        .ok();

    home.ensure_dir()?;
    let lock = home.open_lock()?;
    if lock.try_lock_exclusive().is_err() {
        bail!(
            "another oplan daemon already holds {}",
            home.lock_path().display()
        );
    }

    // 127.0.0.1 keeps /admin/shutdown and every other route reachable only from this machine.
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("binding {addr}"))?;
    let bound = listener.local_addr()?.port();

    let info = DaemonInfo {
        pid: std::process::id(),
        port: bound,
        version: env!("CARGO_PKG_VERSION").to_owned(),
        started_at: now_unix(),
    };
    home.write_info(&info)?;

    let state = AppState::default().with_health(info.clone());
    let (tx, rx) = std::sync::mpsc::channel();
    let _watcher = match Store::discover(root) {
        Ok(store) => Watcher::start(store.plan_dir(), tx)
            .inspect_err(|e| tracing::warn!("watch disabled: {e}"))
            .ok(),
        Err(_) => {
            tracing::warn!("no .plan/ store found; watch disabled");
            None
        }
    };
    std::thread::spawn(move || {
        for event in rx {
            tracing::debug!(?event, "change");
        }
    });

    ignore_sighup();
    tracing::info!(%addr, pid = info.pid, "oplan daemon serving");

    let result = op_server::serve(listener, state, terminate_signal()).await;

    home.clear_info();
    fs2::FileExt::unlock(&lock).ok();
    result.map_err(Into::into)
}

fn ignore_sighup() {
    if let Ok(mut hup) = signal(SignalKind::hangup()) {
        tokio::spawn(async move { while hup.recv().await.is_some() {} });
    }
}

async fn terminate_signal() {
    // Keep both handles owned for the whole wait so a failure to register one never
    // drops the other (which would revert that signal to its default kill disposition).
    let mut interrupt = signal(SignalKind::interrupt()).ok();
    let mut terminate = signal(SignalKind::terminate()).ok();

    let on_interrupt = async {
        match interrupt.as_mut() {
            Some(sig) => {
                sig.recv().await;
            }
            None => std::future::pending::<()>().await,
        }
    };
    let on_terminate = async {
        match terminate.as_mut() {
            Some(sig) => {
                sig.recv().await;
            }
            None => std::future::pending::<()>().await,
        }
    };

    tokio::select! {
        _ = on_interrupt => {}
        _ = on_terminate => {}
    }
}
