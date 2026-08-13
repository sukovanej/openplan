use std::path::Path;

use anyhow::{Context as _, Result};
use op_client::Client;

use crate::ProjectCommand;
use crate::daemon::daemon_base_url;

// The daemon owns `registry.toml` and is its only writer, so every one of these commands asks it
// rather than edit the file. A command that finds no daemon starts one.
pub fn run(command: ProjectCommand, root: &Path, daemon_url: Option<&str>) -> Result<()> {
    let client = Client::default();
    let base_url = daemon_base_url(&client, daemon_url)?;
    match command {
        ProjectCommand::List => list(&client, &base_url),
        ProjectCommand::Add { path } => add(&client, &base_url, path.as_deref(), root),
        ProjectCommand::Remove { name } => remove(&client, &base_url, &name),
        ProjectCommand::Rename { from, to } => rename(&client, &base_url, &from, &to),
    }
}

fn list(client: &Client, base_url: &str) -> Result<()> {
    let views = client.projects(base_url, op_client::WRITE_TIMEOUT)?;
    if views.is_empty() {
        println!("no projects registered");
        return Ok(());
    }
    let width = views.iter().map(|view| view.name.len()).max().unwrap_or(0);
    for view in &views {
        println!(
            "{:<width$}  {}  {}",
            view.name, view.abbreviation, view.root
        );
        // A demoted project is still registered and still listed. Its reason is the answer to "why
        // does the UI not show this project".
        if let Some(reason) = view.status.reason() {
            println!("!       {reason}");
        }
    }
    Ok(())
}

fn add(client: &Client, base_url: &str, path: Option<&Path>, root: &Path) -> Result<()> {
    // The daemon resolves this to the checkout that can serve it, and its own working directory is
    // not the caller's, so the path it receives has to be absolute.
    let asked = path.unwrap_or(root);
    let path = std::fs::canonicalize(asked)
        .with_context(|| format!("no such directory: {}", asked.display()))?;
    let (view, created) = client.register_project(base_url, &path)?;
    match created {
        true => println!("registered {} at {}", view.name, view.root),
        false => println!("{} already serves {}", view.name, view.root),
    }
    Ok(())
}

fn remove(client: &Client, base_url: &str, name: &str) -> Result<()> {
    client.remove_project(base_url, name)?;
    println!("removed {name}; its files stay on disk");
    Ok(())
}

fn rename(client: &Client, base_url: &str, from: &str, to: &str) -> Result<()> {
    let view = client.rename_project(base_url, from, to)?;
    println!("renamed {from} to {}", view.name);
    Ok(())
}
