mod daemon;
mod mergedriver;
mod serve;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Result, bail};
use clap::{Parser, Subcommand};
use op_api::TaskSummary;
use op_git::Repo;
use op_store::Store;

use daemon::{Control, DEFAULT_PORT, Home};

#[derive(Parser)]
#[command(name = "oplan", version, about = "open-planner — local-first task CLI")]
struct Cli {
    #[arg(long, global = true, default_value = ".")]
    root: PathBuf,
    /// Connect to this daemon URL instead of the machine daemon
    #[arg(long, global = true)]
    daemon: Option<String>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List tasks in the store
    List,
    /// List local git branches
    Branches,
    /// Manage the background daemon and web UI
    Server {
        #[command(subcommand)]
        command: ServerCommand,
    },
    /// Git merge driver for .plan/**.md (git passes %O %A %B)
    MergeDriver {
        ancestor: String,
        current: String,
        other: String,
    },
}

#[derive(Subcommand)]
enum ServerCommand {
    /// Start the daemon detached; a no-op if one is already running
    Start {
        #[arg(long, default_value_t = DEFAULT_PORT)]
        port: u16,
        /// Run in the foreground instead of detaching (also used internally)
        #[arg(long)]
        foreground: bool,
    },
    /// Stop the running daemon, gracefully if it answers
    Stop,
    /// Report daemon status without starting it
    Ping,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<ExitCode> {
    match cli.command {
        Command::List => list(&cli.root).map(|()| ExitCode::SUCCESS),
        Command::Branches => branches(&cli.root).map(|()| ExitCode::SUCCESS),
        Command::Server { command } => server(command, &cli.root, cli.daemon.as_deref()),
        Command::MergeDriver {
            ancestor,
            current,
            other,
        } => Ok(mergedriver::run(&ancestor, &current, &other)),
    }
}

fn server(command: ServerCommand, root: &Path, daemon_url: Option<&str>) -> Result<ExitCode> {
    match command {
        ServerCommand::Start { port, foreground } => {
            if let Some(url) = daemon_url {
                bail!(
                    "--daemon {url} cannot be used with `server start`; start launches the local machine daemon"
                );
            }
            if foreground {
                let runtime = tokio::runtime::Runtime::new()?;
                runtime.block_on(serve::run(Home::resolve()?, port, root))?;
            } else {
                Control::resolve()?.start(port, root)?;
            }
            Ok(ExitCode::SUCCESS)
        }
        ServerCommand::Stop => {
            Control::resolve()?.stop(daemon_url)?;
            Ok(ExitCode::SUCCESS)
        }
        ServerCommand::Ping => {
            let running = Control::resolve()?.ping(daemon_url)?;
            Ok(if running {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
    }
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
