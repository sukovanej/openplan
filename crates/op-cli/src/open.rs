use std::path::Path;

use anyhow::Result;
use op_client::Client;
use op_server::Location;

use crate::daemon::daemon_base_url;
use crate::plan::resolve_project;

pub fn run(root: &Path, daemon_url: Option<&str>) -> Result<()> {
    let client = Client::default();
    // The daemon may bind a port the caller chose, so the URL carries the port it reports.
    let base_url = daemon_base_url(&client, daemon_url)?;
    // The UI shows the projects the daemon serves, so opening it from a fresh clone registers that
    // clone, as a first write does. A caller who borrowed a daemon with --daemon leaves its
    // registry alone.
    if daemon_url.is_none()
        && let Ok(location) = Location::find_or_join(root)
    {
        resolve_project(&client, &base_url, &location, true)?;
    }
    let url = format!("{base_url}/");
    op_browser::open(&url)?;
    println!("opened {url}");
    Ok(())
}
