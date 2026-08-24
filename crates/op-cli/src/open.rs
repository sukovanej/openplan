use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, anyhow, bail};
use op_client::Client;
use op_git::Repo;

use crate::daemon::daemon_base_url;
use crate::plan::resolve_project;

#[cfg(target_os = "macos")]
const DEFAULT_LAUNCHER: &str = "open";
#[cfg(not(target_os = "macos"))]
const DEFAULT_LAUNCHER: &str = "xdg-open";

// A launcher that is the browser itself does not exit until the user closes the window, so past
// this point the command stops waiting and reports success. A launcher that cannot run, or that
// refuses the URL, answers well inside it.
const LAUNCH_DEADLINE: Duration = Duration::from_secs(1);

pub fn run(root: &Path, daemon_url: Option<&str>) -> Result<()> {
    let client = Client::default();
    // The daemon may bind port 0 or a port the caller chose, so the URL carries the port it
    // reports, never the default one.
    let base_url = daemon_base_url(&client, daemon_url)?;
    // The UI shows the projects the daemon serves, so opening it from a fresh checkout has to
    // register that checkout, exactly as a first write does. A caller outside a repository has
    // nothing to register, and one who borrowed a daemon with --daemon must leave its registry
    // alone.
    if daemon_url.is_none()
        && let Ok(repo) = Repo::discover(root)
    {
        resolve_project(&client, &base_url, &repo, root, true)?;
    }
    let url = format!("{base_url}/");
    launch(&url)?;
    println!("opened {url}");
    Ok(())
}

struct Launcher {
    program: String,
    args: Vec<String>,
}

fn launch(url: &str) -> Result<()> {
    let mut failure = anyhow!("no command to open {url}; set $BROWSER");
    for launcher in launchers(url) {
        match launcher.spawn() {
            Ok(child) => return confirm(child, &launcher.program, url),
            // $BROWSER lists candidates in order of preference, so a name this machine does not
            // have is a reason to try the next one rather than to stop.
            Err(err) => {
                failure = anyhow::Error::new(err).context(format!(
                    "cannot run {}; set $BROWSER to a command that opens a URL",
                    launcher.program
                ));
            }
        }
    }
    Err(failure)
}

impl Launcher {
    fn spawn(&self) -> std::io::Result<Child> {
        Command::new(&self.program)
            .args(&self.args)
            .stdin(Stdio::null())
            // A browser that keeps running holds whatever streams it inherits, so it would write
            // into this command's own output long after the command returned — and hold open the
            // pipes of any caller that captures that output.
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
    }
}

fn confirm(mut child: Child, program: &str, url: &str) -> Result<()> {
    let deadline = Instant::now() + LAUNCH_DEADLINE;
    loop {
        match child
            .try_wait()
            .context("waiting for the browser launcher")?
        {
            Some(status) if status.success() => return Ok(()),
            Some(status) => bail!(
                "{program} did not open {url} ({status}); set $BROWSER to a command that opens a \
                 URL"
            ),
            None if Instant::now() >= deadline => return Ok(()),
            None => std::thread::sleep(Duration::from_millis(20)),
        }
    }
}

// $BROWSER holds a colon-separated list of commands, each tried in turn, and each placing the URL
// where it spells `%s` or else at the end.
fn launchers(url: &str) -> Vec<Launcher> {
    let browser = std::env::var("BROWSER").unwrap_or_default();
    let listed: Vec<Launcher> = browser
        .split(':')
        .filter(|entry| !entry.trim().is_empty())
        .map(|entry| launcher(entry, url))
        .collect();
    if listed.is_empty() {
        return vec![launcher(DEFAULT_LAUNCHER, url)];
    }
    listed
}

fn launcher(entry: &str, url: &str) -> Launcher {
    let words: Vec<&str> = entry.split_whitespace().collect();
    let spells_url = words.iter().any(|word| word.contains("%s"));
    let mut placed = words.iter().map(|word| word.replace("%s", url));
    let program = placed.next().unwrap_or_else(|| DEFAULT_LAUNCHER.to_owned());
    let mut args: Vec<String> = placed.collect();
    if !spells_url {
        args.push(url.to_owned());
    }
    Launcher { program, args }
}
