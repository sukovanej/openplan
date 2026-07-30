use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use fs2::FileExt as _;
use op_api::ChangeEvent;
use op_git::Repo;
use op_server::AppState;
use op_store::{Config, STORE_DIR, Store, StoreError};
use op_watch::{Change, Watcher};
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

    serve(home, port, root).await.inspect_err(|err| {
        // With the subscriber up, lifecycle failures go through tracing for consistent formatting
        // instead of the CLI's plain `error: ...` stderr line; fall back to stderr when ERROR is
        // filtered out (e.g. RUST_LOG=off) so a failed startup is never silent.
        if tracing::enabled!(tracing::Level::ERROR) {
            tracing::error!(error = format!("{err:#}"), "daemon exited with error");
        } else {
            eprintln!("error: {err:#}");
        }
    })
}

async fn serve(home: Home, port: u16, root: &Path) -> Result<()> {
    // `--root` defaults to `.`, and a relative path keeps testing as present after the directory it
    // names is deleted — `Path::new(".").is_dir()` stays true once the cwd is unlinked — which would
    // hide the vanished root from `root_removed`.
    let root = &root.canonicalize().unwrap_or_else(|_| root.to_path_buf());

    // Reads route through the branch-aware index, which needs a repo; there is no degraded no-repo
    // serving mode. Fail before binding or taking the lifetime lock so a bad root leaves nothing
    // half-started.
    // A store with no valid abbreviation has no id space above its files, so there is nothing to
    // degrade into: the daemon refuses to start rather than serve keys the store does not claim.
    let store = Store::discover(root).map_err(|err| match err {
        StoreError::StoreMissing => {
            anyhow!("no {STORE_DIR} task store found at {}", root.display())
        }
        other => other.into(),
    })?;
    let repo = Repo::discover(root).with_context(|| {
        format!(
            "oplan serve requires a git repository; none found at {}",
            root.display()
        )
    })?;

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
        repo: Some(
            repo.git_common_dir()
                .canonicalize()
                .unwrap_or_else(|_| repo.git_common_dir())
                .display()
                .to_string(),
        ),
    };
    home.write_info(&info)?;

    let state =
        AppState::new(repo.clone(), store.clone(), store.abbreviation()).with_health(info.clone());
    if let Err(err) = state
        .index
        .lock()
        .expect("index mutex poisoned")
        .rebuild(&repo, &store)
    {
        tracing::warn!(%err, "initial matrix build failed");
    }
    let (tx, rx) = std::sync::mpsc::channel();
    // Watcher::start scans every branch and hashes each worktree's task files; keep that off the
    // async runtime thread.
    let watch_repo = repo.clone();
    let watch_store = store.clone();
    let _watcher = tokio::task::spawn_blocking(move || Watcher::start(watch_repo, watch_store, tx))
        .await
        .ok()
        .and_then(|started| {
            started
                .inspect_err(|e| tracing::warn!("watch disabled: {e}"))
                .ok()
        });
    // Bridge the watcher's changes onto the broadcast so /api/events fans them out to every
    // connected UI, alongside the writes the API handlers publish directly. The watcher reports a
    // task by number, the file layer's spelling; the key it renders as is the daemon's to decide.
    let bridged = state.clone();
    let config_root = store.root().to_path_buf();
    std::thread::spawn(move || {
        for change in rx {
            tracing::debug!(?change, "watcher change forwarded");
            match change {
                Change::Task { number, branch } => {
                    let _ = bridged.event_sender().send(ChangeEvent::TaskChanged {
                        id: bridged.abbreviation().format_key(number),
                        branch,
                    });
                }
                Change::Config => reload_config(&bridged, &config_root),
            }
        }
    });

    ignore_sighup();
    tracing::info!(%addr, pid = info.pid, "oplan daemon serving");

    let result = op_server::serve(listener, state, shutdown_signal(root.to_path_buf())).await;

    home.clear_info();
    fs2::FileExt::unlock(&lock).ok();
    result.map_err(Into::into)
}

// A valid change applies live and every key re-renders. One that leaves the file missing or invalid
// stops the daemon exactly as it would have refused to start, so no store is ever served without an
// abbreviation — including after a checkout that removes the file.
fn reload_config(state: &AppState, root: &std::path::Path) {
    match Config::read(root) {
        Ok(config) => {
            state.set_abbreviation(config.abbreviation);
            let _ = state.event_sender().send(ChangeEvent::ConfigChanged);
            tracing::info!(abbreviation = %config.abbreviation, "store abbreviation reloaded");
        }
        Err(err) => {
            tracing::error!(%err, "the store no longer names an abbreviation; stopping");
            state.stop();
        }
    }
}

fn ignore_sighup() {
    if let Ok(mut hup) = signal(SignalKind::hangup()) {
        tokio::spawn(async move { while hup.recv().await.is_some() {} });
    }
}

async fn shutdown_signal(root: PathBuf) {
    tokio::select! {
        _ = terminate_signal() => {}
        _ = root_removed(root) => {}
    }
}

// A worktree deleted under a running daemon leaves it serving a tree that no longer exists — no
// writes can land and its watch set is gone. Two consecutive misses, so a root that momentarily
// looks absent does not take the daemon down.
async fn root_removed(root: PathBuf) {
    const POLL: Duration = Duration::from_secs(5);
    let mut misses = 0;
    loop {
        tokio::time::sleep(POLL).await;
        misses = if root.is_dir() { 0 } else { misses + 1 };
        if misses == 2 {
            tracing::warn!(root = %root.display(), "serving root is gone; shutting down");
            return;
        }
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
