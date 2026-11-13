use std::process::Command;
use std::time::Duration;

use op_api::{BackendKind, Forge};

use crate::Location;

const GLAB_TIMEOUT: Duration = Duration::from_secs(5);

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

// `glab` writes the report to stderr in some versions and to stdout in others.
fn glab_auth_status() -> Option<String> {
    let mut command = Command::new("glab");
    command.args(["auth", "status"]).env("NO_COLOR", "1");
    match crate::pull_requests::run(command, GLAB_TIMEOUT) {
        Ok(output) => Some(format!("{}\n{}", output.stdout, output.stderr)),
        Err(err) => {
            if err.kind() == std::io::ErrorKind::TimedOut {
                tracing::warn!("glab auth status gave no answer in {GLAB_TIMEOUT:?}");
            }
            None
        }
    }
}
