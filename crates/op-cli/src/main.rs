mod mergedriver;
mod serve;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::Result;
use clap::{Parser, Subcommand};
use op_api::TaskSummary;
use op_git::Repo;
use op_store::Store;

#[derive(Parser)]
#[command(name = "oplan", version, about = "open-planner — local-first task CLI")]
struct Cli {
    #[arg(long, global = true, default_value = ".")]
    root: PathBuf,
    #[arg(long, global = true, default_value = "http://127.0.0.1:7373")]
    daemon: String,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List tasks in the store
    List,
    /// List local git branches
    Branches,
    /// Check whether the daemon is reachable
    Ping,
    /// Run the realtime daemon and web UI
    Serve {
        #[arg(long, default_value_t = 7373)]
        port: u16,
    },
    /// Git merge driver for .plan/**.md (git passes %O %A %B)
    MergeDriver {
        ancestor: String,
        current: String,
        other: String,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let outcome: Result<()> = match cli.command {
        Command::List => list(&cli.root),
        Command::Branches => branches(&cli.root),
        Command::Ping => {
            ping(&cli.daemon);
            Ok(())
        }
        Command::Serve { port } => run_serve(port),
        Command::MergeDriver {
            ancestor,
            current,
            other,
        } => return mergedriver::run(&ancestor, &current, &other),
    };

    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    }
}

fn run_serve(port: u16) -> Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(serve::run(port))
}

fn list(root: &Path) -> Result<()> {
    let store = Store::discover(root)?;
    let ids = store.task_ids()?;
    if ids.is_empty() {
        println!("no tasks yet");
        return Ok(());
    }

    for id in ids {
        match store.read(&id) {
            Ok(task) => {
                let summary = TaskSummary {
                    title: task.title().unwrap_or_default(),
                    status: task.frontmatter.status,
                    parent: task.frontmatter.parent,
                    id,
                };
                let status = format!("{:?}", summary.status);
                println!("{:<28} {status:<11} {}", summary.id, summary.title);
            }
            Err(err) => eprintln!("{id}: {err}"),
        }
    }
    Ok(())
}

fn branches(root: &Path) -> Result<()> {
    let repo = Repo::discover(root)?;
    for branch in repo.local_branches()? {
        println!("{branch}");
    }
    Ok(())
}

fn ping(daemon: &str) {
    let _client = reqwest::Client::new();
    println!("daemon endpoint: {daemon} (probe not yet implemented)");
}
