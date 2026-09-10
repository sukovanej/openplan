use std::path::Path;

use anyhow::Result;
use op_client::Client;
use op_git::Repo;

use crate::daemon::daemon_base_url;
use crate::plan::resolve_project;

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
    op_browser::open(&url)?;
    println!("opened {url}");
    Ok(())
}
