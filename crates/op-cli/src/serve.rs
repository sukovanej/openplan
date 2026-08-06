use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use fs2::FileExt as _;
use op_server::{AppState, Project, ProjectEntry, ProjectRegistry, REGISTRY_FILE};
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
    // hide a vanished root from the daemon's per-project watchdog.
    let root = &root.canonicalize().unwrap_or_else(|_| root.to_path_buf());

    home.ensure_dir()?;
    let lock = home.open_lock()?;
    if lock.try_lock_exclusive().is_err() {
        bail!(
            "another oplan daemon already holds {}",
            home.lock_path().display()
        );
    }

    // Behind the lifetime lock, so registration cannot race a second starter.
    let registry_path = home.dir().join(REGISTRY_FILE);
    let registry = ProjectRegistry::read(&registry_path)?.unwrap_or_default();
    let state = AppState::new(open_projects(registry.entries())).with_registry(registry_path);

    // 127.0.0.1 keeps /admin/shutdown and every other route reachable only from this machine.
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("binding {addr}"))?;
    let bound = listener.local_addr()?.port();

    // `--root` names the project the routes that carry no project segment answer for, and adds it
    // when it is new. It is not a precondition: a root that cannot be served is one degraded
    // project, and the daemon keeps serving every other one — zero of them included. Registering
    // starts a watcher, which scans every branch, so it belongs off the async runtime thread.
    let registering = state.clone();
    let at = root.clone();
    let served = tokio::task::spawn_blocking(move || register_root(&registering, &at))
        .await
        .ok()
        .flatten();

    let info = DaemonInfo {
        pid: std::process::id(),
        port: bound,
        version: env!("CARGO_PKG_VERSION").to_owned(),
        started_at: now_unix(),
        repo: served
            .as_ref()
            .map(|project| project.git_common_dir().to_owned()),
    };
    home.write_info(&info)?;
    let state = state.with_health(info.clone());

    // Watchers scan every branch and hash each worktree's task files; keep that off the async
    // runtime thread. `--root` already started its own during registration.
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

fn register_root(state: &AppState, root: &Path) -> Option<Arc<Project>> {
    match state.register(root) {
        Ok((view, _)) => {
            state.set_default_project(&view.name);
            state.project(&view.name)
        }
        Err(err) => {
            tracing::error!(
                root = %root.display(),
                error = format!("{err:#}"),
                "--root is not being served"
            );
            None
        }
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

// A registered path whose store or repository cannot be opened is skipped, not fatal: one broken
// checkout must not take the task UI away from every other project on the machine. A name is the
// coordinate every route resolves through, so a hand-written duplicate is skipped for the same
// reason — the second entry would otherwise silently shadow the first.
fn open_projects(entries: &[ProjectEntry]) -> Vec<Project> {
    let mut taken = std::collections::BTreeSet::new();
    entries
        .iter()
        .filter(|entry| {
            taken.insert(entry.name.clone()) || {
                tracing::error!(project = %entry.name, "skipping project: the name is already taken");
                false
            }
        })
        .filter_map(
            |entry| match Project::open(entry.name.clone(), entry.path.clone()) {
                Ok(project) => Some(project),
                Err(err) => {
                    tracing::error!(
                        project = %entry.name,
                        path = %entry.path.display(),
                        error = format!("{err:#}"),
                        "skipping project"
                    );
                    None
                }
            },
        )
        .collect()
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
