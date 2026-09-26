use std::net::SocketAddr;
use std::os::unix::process::CommandExt as _;
use std::path::Path;
use std::process::{Command, ExitCode};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use fs2::FileExt as _;
use op_server::{AppState, ProjectRegistry, REGISTRY_FILE, open_projects};
use tokio::signal::unix::{SignalKind, signal};
use tracing_subscriber::EnvFilter;

use op_api::{DaemonInfo, StopReason};

use crate::home::Home;
use crate::now_unix;
use crate::update::{self, Updater, Updates};

// An update check can still be downloading when the daemon stops; the stop does not wait for it.
const BLOCKING_GRACE: Duration = Duration::from_secs(1);

enum Exit {
    Stopped,
    Updated { port: u16 },
}

// The daemon is a re-exec of whichever binary asked for one, so every binary that can start a
// daemon answers this argument before its own parsing. A flag rather than an environment variable,
// so a git command the daemon runs does not inherit it.
pub const SERVE_ARG: &str = "--serve-daemon";

pub fn serve_request(args: impl IntoIterator<Item = String>) -> Option<u16> {
    let mut args = args.into_iter().skip(1);
    if args.next().as_deref() != Some(SERVE_ARG) {
        return None;
    }
    args.next()?.parse().ok()
}

pub fn serve_if_requested(
    args: impl IntoIterator<Item = String>,
    updates: Updates,
) -> Option<ExitCode> {
    let port = serve_request(args)?;
    Some(match Home::resolve() {
        Ok(home) => serve(home, port, updates),
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    })
}

pub fn serve(home: Home, port: u16, updates: Updates) -> ExitCode {
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::FAILURE;
        }
    };
    // The updater's blocking HTTP client panics when it is dropped on the runtime, so this frame
    // keeps the last reference until the runtime is gone.
    let updater = Updater::for_this_binary(updates).map(Arc::new);
    // `run` reports its own failure through tracing, so the caller must not print the cause again.
    let exit = runtime.block_on(run(home, port, updater.clone()));
    runtime.shutdown_timeout(BLOCKING_GRACE);
    match (exit, &updater) {
        (Ok(Exit::Updated { port }), Ok(updater)) => start_again(updater.exe(), port),
        (Ok(_), _) => ExitCode::SUCCESS,
        (Err(_), _) => ExitCode::FAILURE,
    }
}

// The new executable replaces this process, so it keeps the pid, the log, and the port the web UI
// reconnects to.
fn start_again(exe: &Path, port: u16) -> ExitCode {
    let err = Command::new(exe)
        .args([SERVE_ARG, &port.to_string()])
        .exec();
    tracing::error!(error = %err, exe = %exe.display(), "cannot start the updated daemon");
    ExitCode::FAILURE
}

async fn run(home: Home, port: u16, updater: Result<Arc<Updater>, String>) -> Result<Exit> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .try_init()
        .ok();

    bind_and_serve(home, port, updater)
        .await
        .inspect_err(|err| {
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

async fn bind_and_serve(
    home: Home,
    port: u16,
    updater: Result<Arc<Updater>, String>,
) -> Result<Exit> {
    home.ensure_dir()?;
    let lock = home.open_lock()?;
    if lock.try_lock_exclusive().is_err() {
        bail!(
            "another openplan daemon already holds {}",
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

    let info = DaemonInfo {
        pid: std::process::id(),
        port: bound,
        version: env!("CARGO_PKG_VERSION").to_owned(),
        started_at: now_unix(),
    };
    home.write_info(&info)?;
    update::confirm_install(&home);
    let state = state.with_health(info.clone());

    state.start_projects();

    ignore_sighup();
    tracing::info!(%addr, pid = info.pid, version = info.version, "openplan daemon serving");

    let home = Arc::new(home);
    let updates = tokio::spawn(update::keep_updated(
        updater,
        Arc::clone(&home),
        state.clone(),
    ));
    let result = op_server::serve(listener, state.clone(), terminate_signal()).await;
    updates.abort();

    home.clear_info();
    fs2::FileExt::unlock(&lock).ok();
    result?;
    Ok(match state.stop_reason() {
        Some(StopReason::Update) => Exit::Updated { port: bound },
        _ => Exit::Stopped,
    })
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
