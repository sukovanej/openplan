use std::path::Path;
use std::process::Command;

use op_client::Client;
use op_daemon::{Home, base_url, default_port};

use crate::cli;

#[derive(Debug, thiserror::Error)]
pub enum Unreachable {
    #[error("Could not resolve the openplan home directory.\n\n{0}")]
    NoHome(String),
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
    if let Some(url) = answering(&home, &client) {
        return Ok(url);
    }

    let search = cli::Search::from_env(resources.map(Path::to_path_buf));
    let binary = search.find().ok_or_else(|| Unreachable::NoBinary {
        places: listed(&search.places()),
        override_name: cli::OVERRIDE,
    })?;
    start(&binary)?;

    answering(&home, &client).ok_or_else(|| Unreachable::Silent {
        binary: binary.display().to_string(),
        port: default_port(),
        log: home.log_path().display().to_string(),
    })
}

// `server start` blocks until the daemon answers /health or its own deadline expires, so the caller
// needs no second readiness wait.
fn start(binary: &Path) -> Result<(), Unreachable> {
    let failure = |reason: String| Unreachable::StartFailed {
        binary: binary.display().to_string(),
        reason,
    };
    let output = Command::new(binary)
        .args(["server", "start"])
        .output()
        .map_err(|err| failure(err.to_string()))?;
    if output.status.success() {
        return Ok(());
    }
    Err(failure(
        String::from_utf8_lossy(&output.stderr).trim().to_owned(),
    ))
}

// A live daemon answers /health with its own identity; requiring the pid to match daemon.json
// rejects a stale record whose port was recycled by an unrelated service.
fn answering(home: &Home, client: &Client) -> Option<String> {
    let info = home.read_info()?;
    let url = base_url(info.port);
    let live = client.health(&url)?;
    (live.pid == info.pid).then_some(url)
}

fn listed(places: &[std::path::PathBuf]) -> String {
    places
        .iter()
        .map(|place| format!("  {}", place.display()))
        .collect::<Vec<_>>()
        .join("\n")
}
