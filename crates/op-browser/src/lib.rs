use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, anyhow, bail};

#[cfg(target_os = "macos")]
pub const DEFAULT_LAUNCHER: &str = "open";
#[cfg(not(target_os = "macos"))]
pub const DEFAULT_LAUNCHER: &str = "xdg-open";

// A launcher that is the browser itself does not exit until the user closes the window, so past
// this point the command stops waiting and reports success. A launcher that cannot run, or that
// refuses the URL, answers well inside it.
const LAUNCH_DEADLINE: Duration = Duration::from_secs(1);

pub struct Launcher {
    pub program: String,
    pub args: Vec<String>,
}

pub fn open(url: &str) -> Result<()> {
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
        return default_launchers(under_wsl(), url);
    }
    listed
}

pub fn default_launchers(wsl: bool, url: &str) -> Vec<Launcher> {
    let mut candidates = Vec::new();
    if wsl {
        candidates.push(windows_launcher(url));
    }
    candidates.push(launcher(DEFAULT_LAUNCHER, url));
    candidates
}

// A distribution under WSL often omits xdg-open, and the browser the user sees is the Windows one.
// WSL can also run with interop off or with the Windows PATH detached, which leaves xdg-open as
// the only way out, so both stay in the list.
fn windows_launcher(url: &str) -> Launcher {
    // `cmd.exe /C start` reads the URL as a command line: an `&` in it starts a second command,
    // `%NAME%` expands, and the first quoted word is the window title rather than the URL.
    // PowerShell takes the URL as one single-quoted literal, where only the quote needs escaping.
    Launcher {
        program: "powershell.exe".to_owned(),
        args: vec![
            "-NoProfile".to_owned(),
            "-NonInteractive".to_owned(),
            "-Command".to_owned(),
            format!("Start-Process '{}'", url.replace('\'', "''")),
        ],
    }
}

// The environment says nothing under sudo, ssh, or a systemd unit, which all reset it. The kernel
// name is there for every process.
#[cfg(target_os = "linux")]
fn under_wsl() -> bool {
    std::fs::read_to_string("/proc/sys/kernel/osrelease").is_ok_and(|release| names_wsl(&release))
}

#[cfg(not(target_os = "linux"))]
fn under_wsl() -> bool {
    false
}

pub fn names_wsl(release: &str) -> bool {
    let release = release.to_ascii_lowercase();
    release.contains("microsoft") || release.contains("wsl")
}

pub fn launcher(entry: &str, url: &str) -> Launcher {
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
