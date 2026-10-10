use std::cmp::Ordering;
use std::path::Path;

use anyhow::{Context, Result, bail};

use op_api::{DaemonInfo, ProjectView};
use op_daemon::{Control, Started, StopOutcome, UpdateRecord, base_url, default_port, now_unix};
use op_server::{Location, same_path};
use semver::Version;
use serde::Serialize;

pub fn start(port: u16) -> Result<()> {
    let control = Control::resolve()?;
    match control.ensure(port)? {
        // port 0 means "any", so a differing bound port there is expected, not ignored.
        Started::Already(info) if port != 0 && info.port != port => {
            println!(
                "already running (pid {}, port {}); ignoring requested port {}",
                info.pid, info.port, port
            );
        }
        Started::Already(info) => {
            println!("already running (pid {}, port {})", info.pid, info.port);
        }
        Started::Fresh(info) => {
            println!(
                "started (pid {}, port {}); singleton for OPENPLAN_HOME={}",
                info.pid,
                info.port,
                control.home().dir().display()
            );
        }
    }
    Ok(())
}

pub fn restart(port: u16) -> Result<()> {
    match Control::resolve()?.stop()? {
        StopOutcome::NotRunning => {}
        StopOutcome::RemovedStale { pid } => println!("removed stale daemon.json for pid {pid}"),
        StopOutcome::Stopped { pid, port } => println!("stopped (pid {pid}, port {port})"),
    }
    start(port)
}

#[derive(Default, Serialize)]
struct Ping {
    running: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    #[serde(flatten)]
    daemon: Option<DaemonInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    updates: Option<UpdateRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stale_pid: Option<u32>,
}

pub fn ping(override_url: Option<&str>, json: bool) -> Result<bool> {
    let ping = probe(override_url)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&ping)?);
    } else {
        print_ping(&ping);
    }
    Ok(ping.running)
}

fn probe(override_url: Option<&str>) -> Result<Ping> {
    let client = op_client::Client::default();
    if let Some(url) = override_url {
        return Ok(Ping {
            running: client.health(url.trim_end_matches('/')).is_some(),
            url: Some(url.to_owned()),
            ..Ping::default()
        });
    }
    let control = Control::resolve()?;
    Ok(match control.recorded() {
        Some(info) if op_daemon::serves(&client, &info) => Ping {
            running: true,
            daemon: Some(info),
            updates: Some(control.home().read_update()),
            ..Ping::default()
        },
        Some(info) => Ping {
            stale_pid: Some(info.pid),
            ..Ping::default()
        },
        None => Ping::default(),
    })
}

fn print_ping(ping: &Ping) {
    match ping {
        Ping {
            url: Some(url),
            running: true,
            ..
        } => println!("running (daemon at {url})"),
        Ping { url: Some(url), .. } => println!("not running (no openplan daemon at {url})"),
        Ping {
            daemon: Some(info),
            updates,
            ..
        } => {
            let uptime = fmt_uptime(now_unix().saturating_sub(info.started_at));
            println!(
                "running (pid {}, port {}, up {}, v{})",
                info.pid, info.port, uptime, info.version
            );
            if let Some(updates) = updates {
                print_updates(updates);
            }
        }
        Ping {
            stale_pid: Some(pid),
            ..
        } => println!("not running (stale daemon.json for pid {pid})"),
        _ => println!("not running"),
    }
}

pub fn stop(override_url: Option<&str>) -> Result<()> {
    if let Some(url) = override_url {
        let base = url.trim_end_matches('/');
        if op_client::Client::default().shutdown(base) {
            println!("stopping (daemon at {url})");
        } else {
            println!("not running (no openplan daemon at {url})");
        }
        return Ok(());
    }

    match Control::resolve()?.stop()? {
        StopOutcome::NotRunning => println!("not running"),
        StopOutcome::RemovedStale { pid } => {
            println!("not running (removed stale daemon.json for pid {pid})");
        }
        StopOutcome::Stopped { pid, port } => println!("stopped (pid {pid}, port {port})"),
    }
    Ok(())
}

// The daemon a command that routes through one talks to: the URL the caller named, or the machine
// daemon, started here if it is not up.
pub fn daemon_base_url(client: &op_client::Client, daemon_url: Option<&str>) -> Result<String> {
    match daemon_url {
        Some(url) => {
            let base = url.trim_end_matches('/').to_owned();
            client
                .health(&base)
                .with_context(|| format!("no openplan daemon at {base}"))?;
            Ok(base)
        }
        None => {
            let control = Control::resolve()?;
            let info = control.ensure(default_port()?)?.into_info();
            Ok(base_url(on_this_version(&control, info)?.port))
        }
    }
}

// The CLI and the daemon speak one API, and a release can change it. A newer CLI starts the daemon
// again on itself. An older one refuses, because a restart from it would take the daemon back.
fn on_this_version(control: &Control, info: DaemonInfo) -> Result<DaemonInfo> {
    let ours = Version::parse(env!("CARGO_PKG_VERSION")).context("the built-in version")?;
    let Ok(theirs) = Version::parse(&info.version) else {
        return Ok(info);
    };
    match ours.cmp(&theirs) {
        Ordering::Equal => Ok(info),
        Ordering::Greater => {
            control.stop()?;
            let started = control.ensure(info.port)?.into_info();
            eprintln!("restarted the daemon on openplan {ours}; it ran {theirs}");
            Ok(started)
        }
        Ordering::Less => bail!(
            "this openplan is {ours}, and the daemon runs the newer {theirs}; run the newer \
             openplan, or update this one with `openplan update`"
        ),
    }
}

// Which project the daemon serves these tasks as. A git project matches on the git common
// directory, so every worktree of a repository resolves to the same project; a local one on its
// root.
pub fn project_named(views: &[ProjectView], location: &Location) -> Option<String> {
    views
        .iter()
        .find(
            |view| match (&view.git_common_dir, &location.git_common_dir) {
                (Some(theirs), Some(ours)) => same_path(Path::new(theirs), ours),
                (None, None) => same_path(Path::new(&view.root), &location.root),
                _ => false,
            },
        )
        .map(|view| view.name.clone())
}

fn print_updates(record: &UpdateRecord) {
    if !record.auto {
        println!("automatic updates: off");
    }
    if let Some(check) = &record.last_check {
        let ago = fmt_uptime(now_unix().saturating_sub(check.at));
        println!("last update check {ago} ago: {}", check.result);
    }
}

fn fmt_uptime(secs: u64) -> String {
    let (h, m, s) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    if h > 0 {
        format!("{h}h{m}m{s}s")
    } else if m > 0 {
        format!("{m}m{s}s")
    } else {
        format!("{s}s")
    }
}
