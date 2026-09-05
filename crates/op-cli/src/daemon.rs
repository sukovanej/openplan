use std::path::Path;

use anyhow::{Context, Result};

use op_api::ProjectView;
use op_daemon::{Control, Started, StopOutcome, base_url, default_port, now_unix};
use op_server::same_path;

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

pub fn ping(override_url: Option<&str>) -> Result<bool> {
    let client = op_client::Client::default();
    if let Some(url) = override_url {
        let up = client.health(url.trim_end_matches('/')).is_some();
        if up {
            println!("running (daemon at {url})");
        } else {
            println!("not running (no openplan daemon at {url})");
        }
        return Ok(up);
    }

    match Control::resolve()?.recorded() {
        Some(info) if op_daemon::serves(&client, &info) => {
            let uptime = fmt_uptime(now_unix().saturating_sub(info.started_at));
            println!(
                "running (pid {}, port {}, up {}, v{})",
                info.pid, info.port, uptime, info.version
            );
            Ok(true)
        }
        Some(info) => {
            println!("not running (stale daemon.json for pid {})", info.pid);
            Ok(false)
        }
        None => {
            println!("not running");
            Ok(false)
        }
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
            let info = Control::resolve()?.ensure(default_port())?.into_info();
            Ok(base_url(info.port))
        }
    }
}

// Which project the daemon serves a repository as. Matched on the git common directory, so every
// worktree of that repository resolves to the same project.
pub fn project_named(views: Vec<ProjectView>, repo_dir: &Path) -> Option<String> {
    views
        .into_iter()
        .find(|view| same_path(Path::new(&view.git_common_dir), repo_dir))
        .map(|view| view.name)
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
