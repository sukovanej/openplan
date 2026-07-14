mod daemon;
mod mergedriver;
mod serve;

use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Result, bail};
use clap::{Parser, Subcommand};
use op_api::{CreateTask, Matrix, MatrixCell, TaskSummary, TaskView};
use op_git::Repo;
use op_index::Index;
use op_store::Store;
use op_task::Status;

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
    /// Create a task and print its new id
    Create {
        title: String,
        #[arg(long)]
        parent: Option<String>,
        #[arg(long)]
        status: Option<Status>,
        #[arg(long = "dep")]
        deps: Vec<String>,
        /// Markdown content placed below the title heading
        #[arg(long, conflicts_with = "body_file")]
        body: Option<String>,
        /// Read the content from a file, or `-` for stdin
        #[arg(long = "body-file")]
        body_file: Option<String>,
    },
    /// List tasks in the store
    List {
        #[arg(long)]
        status: Option<Status>,
        #[arg(long)]
        parent: Option<String>,
        #[arg(long)]
        json: bool,
        /// Every task on every local branch (one matrix row per task×branch)
        #[arg(long = "all-branches", conflicts_with_all = ["branch", "parent"])]
        all_branches: bool,
        /// Tasks as they stand on one branch, without checking it out
        #[arg(long)]
        branch: Option<String>,
    },
    /// Print a whole task file, or its metadata as JSON
    Get {
        id: String,
        #[arg(long)]
        json: bool,
        /// Read the task's version on another branch (read-only)
        #[arg(long)]
        branch: Option<String>,
    },
    /// Print a task's metadata (status, parent, deps)
    Show {
        id: String,
        /// Show the per-branch status matrix for this task instead
        #[arg(long)]
        branches: bool,
    },
    /// Set a validated field: status | parent | deps
    Set {
        id: String,
        field: String,
        value: String,
    },
    /// Delete a task file
    Delete {
        id: String,
        #[arg(long)]
        yes: bool,
    },
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
        Command::Create {
            title,
            parent,
            status,
            deps,
            body,
            body_file,
        } => {
            let body = resolve_body(body, body_file)?;
            create(&cli.root, title, parent, status, deps, body).map(|()| ExitCode::SUCCESS)
        }
        Command::List {
            status,
            parent,
            json,
            all_branches,
            branch,
        } => list(
            &cli.root,
            status,
            parent.as_deref(),
            json,
            all_branches,
            branch.as_deref(),
        )
        .map(|()| ExitCode::SUCCESS),
        Command::Get { id, json, branch } => {
            get(&cli.root, &id, json, branch.as_deref()).map(|()| ExitCode::SUCCESS)
        }
        Command::Show { id, branches } => {
            show(&cli.root, &id, branches).map(|()| ExitCode::SUCCESS)
        }
        Command::Set { id, field, value } => {
            set(&cli.root, &id, &field, &value).map(|()| ExitCode::SUCCESS)
        }
        Command::Delete { id, yes } => delete(&cli.root, &id, yes),
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

fn resolve_body(body: Option<String>, body_file: Option<String>) -> Result<Option<String>> {
    match body_file {
        Some(path) if path == "-" => {
            let mut content = String::new();
            std::io::stdin().read_to_string(&mut content)?;
            Ok(Some(content))
        }
        Some(path) => Ok(Some(std::fs::read_to_string(&path)?)),
        None => Ok(body),
    }
}

fn create(
    root: &Path,
    title: String,
    parent: Option<String>,
    status: Option<Status>,
    deps: Vec<String>,
    body: Option<String>,
) -> Result<()> {
    let store = Store::discover(root)?;
    let id = store.create(
        &CreateTask {
            title,
            status,
            parent,
            deps,
            body,
        }
        .into_task(),
    )?;
    println!("{id}");
    Ok(())
}

fn list(
    root: &Path,
    status: Option<Status>,
    parent: Option<&str>,
    json: bool,
    all_branches: bool,
    branch: Option<&str>,
) -> Result<()> {
    if all_branches {
        return list_all_branches(root, status, json);
    }
    if let Some(branch) = branch {
        return list_branch(root, branch, status, parent, json);
    }

    let store = Store::discover(root)?;
    let ids = store.task_ids()?;
    let mut summaries = Vec::new();
    for id in &ids {
        match store.read(id) {
            Ok(task) => {
                if status.is_some_and(|s| task.frontmatter.status != s) {
                    continue;
                }
                if parent.is_some_and(|p| task.frontmatter.parent.as_deref() != Some(p)) {
                    continue;
                }
                summaries.push(TaskSummary::from_task(id.clone(), &task));
            }
            // stderr keeps the diagnostic out of stdout's JSON while still surfacing it.
            Err(err) => eprintln!("{id}: {err}"),
        }
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&summaries)?);
    } else if !summaries.is_empty() {
        print_summaries(&summaries);
    } else if ids.is_empty() {
        println!("no tasks yet");
    } else {
        println!("no matching tasks");
    }
    Ok(())
}

fn list_all_branches(root: &Path, status: Option<Status>, json: bool) -> Result<()> {
    let (_repo, index) = build_index(root)?;
    let cells: Vec<MatrixCell> = index
        .matrix()
        .cells
        .iter()
        .filter(|cell| status.is_none_or(|s| cell.task.status == s))
        .cloned()
        .collect();
    if json {
        println!("{}", serde_json::to_string_pretty(&Matrix { cells })?);
    } else if cells.is_empty() {
        println!("no tasks on any branch");
    } else {
        for cell in &cells {
            let status = format!("{:?}", cell.task.status);
            println!(
                "{:<22} {:<28} {status:<11} {}{}",
                cell.branch,
                cell.task.id,
                cell.task.title,
                cell_flags(cell),
            );
        }
    }
    Ok(())
}

fn list_branch(
    root: &Path,
    branch: &str,
    status: Option<Status>,
    parent: Option<&str>,
    json: bool,
) -> Result<()> {
    let (repo, index) = build_index(root)?;
    ensure_branch(&repo, branch)?;
    let summaries: Vec<TaskSummary> = index
        .branch_summaries(branch)
        .into_iter()
        .filter(|s| status.is_none_or(|st| s.status == st))
        .filter(|s| parent.is_none_or(|p| s.parent.as_deref() == Some(p)))
        .collect();
    if json {
        println!("{}", serde_json::to_string_pretty(&summaries)?);
    } else if summaries.is_empty() {
        println!("no matching tasks");
    } else {
        print_summaries(&summaries);
    }
    Ok(())
}

fn get(root: &Path, id: &str, json: bool, branch: Option<&str>) -> Result<()> {
    if let Some(branch) = branch {
        return get_branch(root, id, branch, json);
    }
    let store = Store::discover(root)?;
    if json {
        let task = store.read(id)?;
        println!(
            "{}",
            serde_json::to_string_pretty(&TaskView::from_task(id.to_owned(), &task))?
        );
    } else {
        // Print the file verbatim; re-serializing would normalize formatting and is a lossy
        // view of what is actually on disk.
        print!("{}", store.read_raw(id)?);
    }
    Ok(())
}

fn get_branch(root: &Path, id: &str, branch: &str, json: bool) -> Result<()> {
    let (repo, index) = build_index(root)?;
    ensure_branch(&repo, branch)?;
    if json {
        match index.effective_view(&repo, id, branch)? {
            Some(view) => println!("{}", serde_json::to_string_pretty(&view)?),
            None => bail!("no such task on branch {branch}: {id}"),
        }
    } else {
        match index.effective_raw(&repo, id, branch)? {
            Some(raw) => print!("{raw}"),
            None => bail!("no such task on branch {branch}: {id}"),
        }
    }
    Ok(())
}

fn show(root: &Path, id: &str, branches: bool) -> Result<()> {
    if branches {
        return show_branches(root, id);
    }
    let store = Store::discover(root)?;
    let task = store.read(id)?;
    let fm = &task.frontmatter;
    println!("id:     {id}");
    println!("title:  {}", task.title().unwrap_or_default());
    println!("status: {}", fm.status.as_str());
    println!("parent: {}", fm.parent.as_deref().unwrap_or("-"));
    println!(
        "deps:   {}",
        if fm.deps.is_empty() {
            "-".to_owned()
        } else {
            fm.deps.join(", ")
        }
    );
    Ok(())
}

fn show_branches(root: &Path, id: &str) -> Result<()> {
    let (_repo, index) = build_index(root)?;
    let Some(view) = index.task_branches(id) else {
        bail!("task not found on any branch: {id}");
    };
    let branch_count: usize = view.versions.iter().map(|v| v.branches.len()).sum();
    let divergent = view.versions.len() > 1;
    println!("id: {}", view.id);
    println!(
        "{} version{} across {} branch{}{}",
        view.versions.len(),
        plural(view.versions.len()),
        branch_count,
        if branch_count == 1 { "" } else { "es" },
        if divergent { " (divergent)" } else { "" },
    );
    for version in &view.versions {
        let short = &version.blob_oid[..version.blob_oid.len().min(12)];
        println!(
            "  {short}  {:<11} {}{}",
            version.summary.status.as_str(),
            version.summary.title,
            version_flags(version),
        );
        println!("    branches: {}", version.branches.join(", "));
    }
    Ok(())
}

fn build_index(root: &Path) -> Result<(Repo, Index)> {
    let repo = Repo::discover(root)?;
    let store = Store::discover(root)?;
    let mut index = Index::new();
    index.rebuild(&repo, &store)?;
    Ok((repo, index))
}

fn ensure_branch(repo: &Repo, branch: &str) -> Result<()> {
    if repo.local_branches()?.iter().any(|b| b == branch) {
        Ok(())
    } else {
        bail!("no such branch: {branch}");
    }
}

fn print_summaries(summaries: &[TaskSummary]) {
    for summary in summaries {
        let status = format!("{:?}", summary.status);
        println!("{:<28} {status:<11} {}", summary.id, summary.title);
    }
}

fn cell_flags(cell: &MatrixCell) -> String {
    let mut flags = Vec::new();
    if cell.dirty {
        flags.push("dirty");
    }
    if cell.conflicted {
        flags.push("conflict");
    }
    if flags.is_empty() {
        String::new()
    } else {
        format!("  [{}]", flags.join(", "))
    }
}

fn version_flags(version: &op_api::TaskVersion) -> String {
    let mut flags = Vec::new();
    if version.dirty {
        flags.push("dirty");
    }
    if version.conflicted {
        flags.push("conflict");
    }
    if flags.is_empty() {
        String::new()
    } else {
        format!("  [{}]", flags.join(", "))
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

fn set(root: &Path, id: &str, field: &str, value: &str) -> Result<()> {
    let store = Store::discover(root)?;
    store.update(id, |task| {
        match field {
            "status" => task.set_status(value.parse().map_err(invalid)?),
            "parent" => task.set_parent(Some(value.to_owned())),
            "deps" => task.set_deps(
                value
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .collect(),
            ),
            other => {
                return Err(invalid(format!(
                    "unknown field {other:?}; expected status | parent | deps"
                )));
            }
        }
        Ok(())
    })?;
    Ok(())
}

fn delete(root: &Path, id: &str, yes: bool) -> Result<ExitCode> {
    let store = Store::discover(root)?;
    if !store.exists(id) {
        bail!("no such task: {id}");
    }
    if !yes && !confirm(id)? {
        println!("aborted");
        return Ok(ExitCode::SUCCESS);
    }
    store.delete(id)?;
    println!("deleted {id}");
    Ok(ExitCode::SUCCESS)
}

fn confirm(id: &str) -> Result<bool> {
    print!("delete {id}? [y/N] ");
    std::io::stdout().flush()?;
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

fn invalid(msg: impl std::fmt::Display) -> op_store::StoreError {
    op_store::StoreError::Invalid(msg.to_string())
}

fn branches(root: &Path) -> Result<()> {
    let repo = Repo::discover(root)?;
    for branch in repo.local_branches()? {
        println!("{branch}");
    }
    Ok(())
}
