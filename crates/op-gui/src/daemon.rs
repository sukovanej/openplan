use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;

use op_client::Client;
use op_daemon::{Home, base_url, default_port};

use crate::cli;

// `openplan server start` waits for the daemon to answer /health, under deadlines of its own that
// add up to about ten seconds. Past that it is not starting anything, and a window with no way out
// is worse than a message.
const START_DEADLINE: Duration = Duration::from_secs(20);

#[derive(Debug, thiserror::Error)]
pub enum Unreachable {
    #[error("Could not resolve the openplan home directory.\n\n{0}")]
    NoHome(String),
    #[error(
        "{override_name} names {path}, which this machine cannot run.\n\nPoint it at the openplan \
         binary, or clear it to search PATH."
    )]
    BadOverride {
        override_name: &'static str,
        path: String,
    },
    #[error(
        "No openplan binary to start the daemon with.\n\nLooked at:\n{places}\n\nSet \
         {override_name} to the binary, or install one with `cargo install --path crates/op-cli`."
    )]
    NoBinary {
        places: String,
        override_name: &'static str,
    },
    #[error("`{binary} server start` failed.\n\n{reason}")]
    StartFailed { binary: String, reason: String },
    #[error(
        "`{binary} server start` returned, but no daemon answers on port {port}.\n\nSee {log}."
    )]
    Silent {
        binary: String,
        port: u16,
        log: String,
    },
}

pub fn url(resources: Option<&Path>) -> Result<String, Unreachable> {
    let home = Home::resolve().map_err(|err| Unreachable::NoHome(format!("{err:#}")))?;
    let client = Client::default();
    if let Some(info) = op_daemon::serving(&home, &client) {
        return Ok(base_url(info.port));
    }

    let binary = cli::Search::from_env(resources.map(Path::to_path_buf))
        .find()
        .map_err(missing)?;
    start(&binary)?;

    op_daemon::serving(&home, &client)
        .map(|info| base_url(info.port))
        .ok_or_else(|| Unreachable::Silent {
            binary: binary.display().to_string(),
            port: home.read_info().map_or_else(default_port, |info| info.port),
            log: home.log_path().display().to_string(),
        })
}

fn missing(missing: cli::Missing) -> Unreachable {
    match missing {
        cli::Missing::Override(path) => Unreachable::BadOverride {
            override_name: cli::OVERRIDE,
            path: path.display().to_string(),
        },
        cli::Missing::Anywhere(places) => Unreachable::NoBinary {
            places: places
                .iter()
                .map(|place| format!("  {}", place.display()))
                .collect::<Vec<_>>()
                .join("\n"),
            override_name: cli::OVERRIDE,
        },
    }
}

// `server start` blocks until the daemon answers /health or its own deadline expires, so the caller
// needs no second readiness wait.
fn start(binary: &Path) -> Result<(), Unreachable> {
    let failure = |reason: String| Unreachable::StartFailed {
        binary: binary.display().to_string(),
        reason,
    };
    let (tx, rx) = mpsc::channel();
    let spawned: PathBuf = binary.to_path_buf();
    std::thread::spawn(move || {
        let _ = tx.send(Command::new(spawned).args(["server", "start"]).output());
    });

    let output = rx
        .recv_timeout(START_DEADLINE)
        .map_err(|_| failure(format!("It did not answer within {START_DEADLINE:?}.")))?
        .map_err(|err| failure(err.to_string()))?;
    if output.status.success() {
        return Ok(());
    }
    Err(failure(
        String::from_utf8_lossy(&output.stderr).trim().to_owned(),
    ))
}
