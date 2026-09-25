use std::path::Path;
use std::process::ExitCode;

use anyhow::Result;
use op_api::SyncView;

use crate::plan::Plan;

pub fn history(
    root: &Path,
    daemon_url: Option<&str>,
    id: Option<&str>,
    before: Option<&str>,
    limit: usize,
    json: bool,
) -> Result<()> {
    let entries = Plan::resolve(root, daemon_url)?.history(id, before, Some(limit))?;
    if json {
        println!("{}", serde_json::to_string_pretty(&entries)?);
        return Ok(());
    }
    if entries.is_empty() {
        println!("no history yet");
        return Ok(());
    }
    for entry in &entries {
        let revision = &entry.revision;
        let agent = match &revision.agent {
            Some(agent) => format!(" via {agent}"),
            None => String::new(),
        };
        let mut lines = revision.message.lines();
        println!(
            "{}  {}  {}{agent}  {}",
            revision.id,
            revision.at,
            revision.author,
            lines.next().unwrap_or_default()
        );
        for line in lines.filter(|line| !line.trim().is_empty()) {
            println!("    {line}");
        }
    }
    if entries.len() == limit
        && let Some(last) = entries.last()
    {
        println!("(older: --before {})", last.revision.id);
    }
    Ok(())
}

pub fn sync(root: &Path, daemon_url: Option<&str>, status: bool, json: bool) -> Result<ExitCode> {
    let plan = Plan::resolve(root, daemon_url)?;
    if status {
        let view = plan.sync_status()?;
        if json {
            println!("{}", serde_json::to_string_pretty(&view)?);
        } else {
            print_status(&view);
        }
        return Ok(match view.error {
            Some(_) => ExitCode::FAILURE,
            None => ExitCode::SUCCESS,
        });
    }
    let result = plan.sync()?;
    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!(
            "received {} revision{}, sent {}{}",
            result.received,
            plural(result.received),
            result.sent,
            if result.merged { ", merged" } else { "" }
        );
    }
    Ok(ExitCode::SUCCESS)
}

fn print_status(view: &SyncView) {
    println!("remote:       {}", view.remote);
    println!(
        "last success: {}",
        view.last_success
            .map_or_else(|| "never".to_owned(), |at| at.to_string())
    );
    println!("ahead:        {}", view.ahead);
    println!("behind:       {}", view.behind);
    if let Some(error) = &view.error {
        println!("!             {error}");
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}
