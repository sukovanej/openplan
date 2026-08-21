use std::process::Command;

use anyhow::{Context as _, Result, bail};
use op_client::Client;

use crate::daemon::daemon_base_url;

#[cfg(target_os = "macos")]
const DEFAULT_LAUNCHER: &str = "open";
#[cfg(not(target_os = "macos"))]
const DEFAULT_LAUNCHER: &str = "xdg-open";

pub fn run(daemon_url: Option<&str>) -> Result<()> {
    // The daemon may bind port 0 or a port the caller chose, so the URL carries the port it
    // reports, never the default one.
    let base_url = daemon_base_url(&Client::default(), daemon_url)?;
    let url = format!("{base_url}/");
    launch(&url)?;
    println!("opened {url}");
    Ok(())
}

fn launch(url: &str) -> Result<()> {
    let (program, args) = launcher();
    let status = Command::new(&program)
        .args(&args)
        .arg(url)
        .status()
        .with_context(|| {
            format!("cannot run {program}; set $BROWSER to a command that opens a URL")
        })?;
    if !status.success() {
        bail!("{program} did not open {url} ({status})");
    }
    Ok(())
}

fn launcher() -> (String, Vec<String>) {
    let browser = std::env::var("BROWSER").unwrap_or_default();
    let mut words = browser.split_whitespace().map(str::to_owned);
    match words.next() {
        Some(program) => (program, words.collect()),
        None => (DEFAULT_LAUNCHER.to_owned(), Vec::new()),
    }
}
