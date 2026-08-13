use std::net::SocketAddr;
use std::path::Path;

use anyhow::{Context, Result, bail};
use fs2::FileExt as _;
use op_server::{AppState, ProjectRegistry, REGISTRY_FILE, open_projects};
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
    home.ensure_dir()?;
    let lock = home.open_lock()?;
    if lock.try_lock_exclusive().is_err() {
        bail!(
            "another oplan daemon already holds {}",
            home.lock_path().display()
        );
    }

    // Behind the lifetime lock, so this read cannot race the registration writes of a second daemon.
    let registry_path = home.dir().join(REGISTRY_FILE);
    let registry = ProjectRegistry::read(&registry_path)?.unwrap_or_default();
    let state = AppState::new(open_projects(registry.entries())).with_registry(registry_path);

    // 127.0.0.1 keeps /admin/shutdown and every other route reachable only from this machine.
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("binding {addr}"))?;
    let bound = listener.local_addr()?.port();

    name_default(&state, root);

    let info = DaemonInfo {
        pid: std::process::id(),
        port: bound,
        version: env!("CARGO_PKG_VERSION").to_owned(),
        started_at: now_unix(),
    };
    home.write_info(&info)?;
    let state = state.with_health(info.clone());

    // Watchers scan every branch and hash each worktree's task files; keep that off the async
    // runtime thread.
    let starting = state.clone();
    tokio::task::spawn_blocking(move || {
        starting.start_watchers();
        warm_indexes(&starting);
    });

    ignore_sighup();
    tracing::info!(%addr, pid = info.pid, "oplan daemon serving");

    let result = op_server::serve(listener, state, terminate_signal()).await;

    home.clear_info();
    fs2::FileExt::unlock(&lock).ok();
    result.map_err(Into::into)
}

// `--root` names the project the routes that carry no project segment answer for. It registers
// nothing: `oplan project add` and the first write from a repository are the only ways in. A root
// that names no registered project leaves the first one of the registry answering them.
fn name_default(state: &AppState, root: &Path) {
    match state.project_at(root) {
        Some(project) => state.set_default_project(&project.name()),
        None => tracing::debug!(root = %root.display(), "--root names no registered project"),
    }
}

// A warm index only saves the first read the work it would do anyway, so it must not delay the port
// answering — with N projects it is N branch walks over the object DB.
fn warm_indexes(state: &AppState) {
    for project in state.projects() {
        if let Err(err) = project.write_index() {
            tracing::warn!(project = %project.name(), %err, "initial matrix build failed");
        }
    }
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
