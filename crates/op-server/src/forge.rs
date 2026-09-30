use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use op_api::{BackendKind, Forge};

use crate::Location;

const GLAB_TIMEOUT: Duration = Duration::from_secs(5);
const GLAB_POLL: Duration = Duration::from_millis(25);

// The repository of the remote the project syncs with. A project on a local directory syncs with
// none.
pub(crate) fn of_project(location: &Location) -> Option<Forge> {
    if location.kind != BackendKind::Git {
        return None;
    }
    let url = op_backend_git::remote_url(&location.root, op_backend_git::DEFAULT_REMOTE)?;
    Forge::of_remote(&url, is_gitlab_host)
}

// A GitLab that a company runs has a host of its own. The person logged in to it with `glab`, so
// `glab` knows it.
fn is_gitlab_host(host: &str) -> bool {
    glab_auth_status()
        .is_some_and(|status| op_forge::glab_hosts(&status).contains(&host.to_owned()))
}

// `glab` writes the report to stderr in some versions and to stdout in others. The report is a few
// lines, so the pipes hold it while this waits.
fn glab_auth_status() -> Option<String> {
    let mut child = Command::new("glab")
        .args(["auth", "status"])
        .env("NO_COLOR", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    let deadline = Instant::now() + GLAB_TIMEOUT;
    while child.try_wait().ok()?.is_none() {
        if Instant::now() >= deadline {
            tracing::warn!("glab auth status gave no answer in {GLAB_TIMEOUT:?}");
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(GLAB_POLL);
    }
    let output = child.wait_with_output().ok()?;
    Some(format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ))
}
