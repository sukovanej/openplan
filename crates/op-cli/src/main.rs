mod author;
mod daemon;
mod mergedriver;
mod open;
mod plan;
mod project;
mod serve;
mod tag;

use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context as _, Result, bail};
use clap::{Parser, Subcommand};
use op_api::{
    BranchMark, ChangeKind, Comment, CreateComment, CreateTask, Field, FieldError, FieldUpdate,
    MatrixCell, Metadata, SearchHit, TaskListItem, TaskPatch, TaskTree, list_item_cmp,
};
use op_git::Repo;
use op_lint::{CreatedSource, Diagnostic, Snapshot};
use op_server::canonical;
use op_store::Store;
use op_task::tag::Color;
use op_task::{Status, Timestamp, rank};

use daemon::{Control, Home};
use plan::Plan;

#[derive(Parser)]
#[command(
    name = "openplan",
    version,
    about = "open-planner — local-first task CLI"
)]
struct Cli {
    /// Directory the command works in [default: the current directory]
    #[arg(long, global = true)]
    root: Option<PathBuf>,
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
        #[arg(long = "dependency")]
        dependencies: Vec<String>,
        /// Assign a registered tag; repeat for more
        #[arg(long = "tag")]
        tags: Vec<String>,
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
    /// Find tasks whose title, body, or frontmatter contains the query, on any branch
    Search {
        query: String,
        #[arg(long)]
        json: bool,
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
    /// Append an entry to a task's comment log
    Comment {
        id: String,
        /// The comment text
        #[arg(conflicts_with = "body_file")]
        text: Option<String>,
        /// Read the text from a file, or `-` for stdin
        #[arg(long = "body-file")]
        body_file: Option<String>,
    },
    /// Print a task's comment log, oldest first
    Comments {
        id: String,
        #[arg(long)]
        json: bool,
        /// Read the log on another branch (read-only)
        #[arg(long, conflicts_with = "all_branches")]
        branch: Option<String>,
        /// Every branch's log, merged by timestamp and labelled with its branch
        #[arg(long = "all-branches")]
        all_branches: bool,
    },
    /// Print a task's metadata (status, parent, dependencies, tags)
    Show {
        id: String,
        /// Show the per-branch status matrix for this task instead
        #[arg(long)]
        branches: bool,
    },
    /// Print the subtask hierarchy rooted at a task
    Tree {
        id: String,
        /// Limit the descent (1 = direct children only); unbounded when omitted
        #[arg(long)]
        depth: Option<usize>,
        #[arg(long)]
        json: bool,
    },
    /// Reparent and/or reorder a task among its siblings (rank)
    Move {
        id: String,
        /// New parent id; "" or "-" moves the task to the top level. Omit to keep the current parent
        #[arg(long)]
        parent: Option<String>,
        /// Place the task immediately before this sibling
        #[arg(long, conflicts_with = "after")]
        before: Option<String>,
        /// Place the task immediately after this sibling
        #[arg(long)]
        after: Option<String>,
    },
    /// Set a validated field: status | parent | dependencies | tags
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
    /// Open the realtime web UI in the default browser
    Open,
    /// Check task files (frontmatter, references, cycles, duplicate numbers); never starts a daemon
    Lint {
        /// Restrict the report and --fix to these tasks (file paths or keys); the whole store is always scanned
        targets: Vec<String>,
        #[arg(long)]
        json: bool,
        /// Apply the derivable fixes in place, then re-check
        #[arg(long)]
        fix: bool,
    },
    /// Manage the tags tasks on this branch can carry
    Tag {
        #[command(subcommand)]
        command: TagCommand,
    },
    /// Manage the repositories the daemon serves
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
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
enum TagCommand {
    /// Register a tag and print the name it normalizes to
    Create {
        name: String,
        #[arg(long)]
        color: Option<Color>,
        #[arg(long = "desc")]
        description: Option<String>,
    },
    /// List every tag this branch registers
    List {
        #[arg(long)]
        json: bool,
    },
    /// Print one tag (name, display name, color, description)
    Show {
        name: String,
        #[arg(long)]
        json: bool,
    },
    /// Set a validated field: color | desc
    Set {
        name: String,
        field: String,
        value: String,
    },
    /// Rename a tag and rewrite the tasks on this branch that carry the old name
    Rename { from: String, to: String },
    /// Delete a tag file
    Delete {
        name: String,
        /// Delete the tag even while tasks on this branch carry it; each of those tasks keeps a
        /// name this branch does not register, and refuses every write until the name goes
        #[arg(long)]
        force: bool,
        #[arg(long)]
        yes: bool,
    },
    /// Print the color names a tag can take
    Colors,
}

#[derive(Subcommand)]
enum ProjectCommand {
    /// List every registered project, with the reason a demoted one is not served
    List,
    /// Register a repository; defaults to --root
    Add { path: Option<PathBuf> },
    /// Drop a project from the registry; its files stay on disk
    Remove { name: String },
    /// Give a project a new name; its URLs change with it
    Rename { from: String, to: String },
}

#[derive(Subcommand)]
enum ServerCommand {
    /// Start the daemon detached; a no-op if one is already running
    Start {
        #[arg(long, default_value_t = daemon::default_port())]
        port: u16,
        /// Run in the foreground instead of detaching (also used internally)
        #[arg(long)]
        foreground: bool,
    },
    /// Stop the running daemon, gracefully if it answers
    Stop,
    /// Stop the running daemon, then start a fresh one
    Restart {
        #[arg(long, default_value_t = daemon::default_port())]
        port: u16,
    },
    /// Report daemon status without starting it
    Ping,
    /// Print the HTTP API's OpenAPI 3.1 spec to stdout
    Openapi,
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
    let daemon_url = cli.daemon.as_deref();
    // Every command works in the current directory unless told otherwise.
    let root = cli.root.as_deref().unwrap_or_else(|| Path::new("."));
    match cli.command {
        Command::Create {
            title,
            parent,
            status,
            dependencies,
            tags,
            body,
            body_file,
        } => {
            let body = resolve_body(body, body_file)?;
            let tags = tag::identities(tags)?;
            create(
                root,
                daemon_url,
                &CreateTask {
                    title,
                    status,
                    parent,
                    dependencies,
                    tags,
                    body,
                },
            )
            .map(|()| ExitCode::SUCCESS)
        }
        Command::List {
            status,
            parent,
            json,
            all_branches,
            branch,
        } => list(
            root,
            daemon_url,
            status,
            parent.as_deref(),
            json,
            all_branches,
            branch.as_deref(),
        )
        .map(|()| ExitCode::SUCCESS),
        Command::Search { query, json } => {
            search(root, daemon_url, &query, json).map(|()| ExitCode::SUCCESS)
        }
        Command::Get { id, json, branch } => {
            get(root, daemon_url, &id, json, branch.as_deref()).map(|()| ExitCode::SUCCESS)
        }
        Command::Comment {
            id,
            text,
            body_file,
        } => {
            let text = resolve_body(text, body_file)?.unwrap_or_default();
            comment(root, daemon_url, &id, &text).map(|()| ExitCode::SUCCESS)
        }
        Command::Comments {
            id,
            json,
            branch,
            all_branches,
        } => comments(root, daemon_url, &id, json, branch.as_deref(), all_branches)
            .map(|()| ExitCode::SUCCESS),
        Command::Show { id, branches } => {
            show(root, daemon_url, &id, branches).map(|()| ExitCode::SUCCESS)
        }
        Command::Tree { id, depth, json } => {
            tree(root, daemon_url, &id, depth, json).map(|()| ExitCode::SUCCESS)
        }
        Command::Move {
            id,
            parent,
            before,
            after,
        } => move_task(root, daemon_url, &id, parent, before, after).map(|()| ExitCode::SUCCESS),
        Command::Set { id, field, value } => {
            set(root, daemon_url, &id, &field, &value).map(|()| ExitCode::SUCCESS)
        }
        Command::Delete { id, yes } => delete(root, daemon_url, &id, yes),
        Command::Branches => branches(root).map(|()| ExitCode::SUCCESS),
        Command::Open => open::run(root, daemon_url).map(|()| ExitCode::SUCCESS),
        Command::Lint { targets, json, fix } => lint(root, &targets, json, fix),
        Command::Tag { command } => tag::run(command, root, daemon_url).map(|()| ExitCode::SUCCESS),
        Command::Project { command } => {
            project::run(command, root, daemon_url).map(|()| ExitCode::SUCCESS)
        }
        Command::Server { command } => server(command, daemon_url),
        Command::MergeDriver {
            ancestor,
            current,
            other,
        } => Ok(mergedriver::run(&ancestor, &current, &other)),
    }
}

fn server(command: ServerCommand, daemon_url: Option<&str>) -> Result<ExitCode> {
    match command {
        ServerCommand::Start { port, foreground } => {
            reject_remote_override(daemon_url, "start")?;
            if foreground {
                let runtime = tokio::runtime::Runtime::new()?;
                // serve::run reports its own failure through tracing; map it to an exit code
                // rather than let main re-print the cause as a plain `error: ...` line.
                return Ok(match runtime.block_on(serve::run(Home::resolve()?, port)) {
                    Ok(()) => ExitCode::SUCCESS,
                    Err(_) => ExitCode::FAILURE,
                });
            }
            Control::resolve()?.start(port)?;
            Ok(ExitCode::SUCCESS)
        }
        ServerCommand::Stop => {
            Control::resolve()?.stop(daemon_url)?;
            Ok(ExitCode::SUCCESS)
        }
        ServerCommand::Restart { port } => {
            reject_remote_override(daemon_url, "restart")?;
            Control::resolve()?.restart(port)?;
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
        ServerCommand::Openapi => {
            let spec = op_server::openapi()
                .to_pretty_json()
                .context("serialize OpenAPI spec")?;
            println!("{spec}");
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn reject_remote_override(daemon_url: Option<&str>, command: &str) -> Result<()> {
    if let Some(url) = daemon_url {
        bail!(
            "--daemon {url} cannot be used with `server {command}`; {command} operates on the local machine daemon, not a remote one"
        );
    }
    Ok(())
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

fn create(root: &Path, daemon_url: Option<&str>, task: &CreateTask) -> Result<()> {
    let id = Plan::resolve(root, daemon_url)?.create(task)?;
    println!("{id}");
    Ok(())
}

fn list(
    root: &Path,
    daemon_url: Option<&str>,
    status: Option<Status>,
    parent: Option<&str>,
    json: bool,
    all_branches: bool,
    branch: Option<&str>,
) -> Result<()> {
    let plan = Plan::resolve(root, daemon_url)?;
    if all_branches {
        return list_all_branches(&plan, status, json);
    }
    let held = plan.list(branch.unwrap_or(plan.branch()))?;
    let matching: Vec<&TaskListItem> = held
        .iter()
        .filter(|task| status.is_none_or(|s| task.metadata.status() == Some(s)))
        .filter(|task| parent.is_none_or(|p| task.metadata.parent() == Some(p)))
        .collect();

    if json {
        println!("{}", serde_json::to_string_pretty(&matching)?);
    } else if !matching.is_empty() {
        print_tasks(&matching);
    } else if held.is_empty() {
        println!("no tasks yet");
    } else {
        println!("no matching tasks");
    }
    Ok(())
}

fn list_all_branches(plan: &Plan, status: Option<Status>, json: bool) -> Result<()> {
    let mut matrix = plan.matrix()?;
    matrix
        .cells
        .retain(|cell| status.is_none_or(|s| cell.task.metadata.status() == Some(s)));
    if json {
        println!("{}", serde_json::to_string_pretty(&matrix)?);
    } else if matrix.cells.is_empty() {
        println!("no tasks on any branch");
    } else {
        for cell in &matrix.cells {
            let status = status_label(&cell.task.metadata);
            println!(
                "{:<22} {:<10} {status:<11} {}{}",
                cell.branch,
                cell.task.id,
                cell.task.title,
                cell_flags(cell),
            );
        }
    }
    Ok(())
}

fn search(root: &Path, daemon_url: Option<&str>, query: &str, json: bool) -> Result<()> {
    let hits = Plan::resolve(root, daemon_url)?.search(query)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&hits)?);
    } else if hits.is_empty() {
        println!("no matching tasks");
    } else {
        print_hits(&hits);
    }
    Ok(())
}

fn get(
    root: &Path,
    daemon_url: Option<&str>,
    id: &str,
    json: bool,
    branch: Option<&str>,
) -> Result<()> {
    let plan = Plan::resolve(root, daemon_url)?;
    let detail = plan.get(id, branch.unwrap_or(plan.branch()))?;
    if json {
        println!("{}", serde_json::to_string_pretty(&detail)?);
    } else {
        // The daemon holds parsed state, not the bytes it parsed, so this is a canonical rendering
        // of the task and not a copy of the file. A field it could not parse has no canonical form
        // and is left out of the rendering rather than guessed at, so it is reported instead.
        for problem in detail.metadata.problems() {
            eprintln!("{id}: {problem}");
        }
        print!(
            "{}",
            op_api::render_task_file(&detail.metadata, &detail.body, &detail.comments)?
        );
    }
    Ok(())
}

fn comment(root: &Path, daemon_url: Option<&str>, id: &str, text: &str) -> Result<()> {
    if text.trim().is_empty() {
        bail!("a comment needs text");
    }
    let tasks = Plan::resolve(root, daemon_url)?;
    let entry = CreateComment {
        text: text.trim_end_matches('\n').to_owned(),
        author: author::author(root)?,
        agent: author::agent(),
    };
    let written = tasks.comment(id, &entry)?;
    println!("{id}: {}", heading_of(&written));
    Ok(())
}

fn comments(
    root: &Path,
    daemon_url: Option<&str>,
    id: &str,
    json: bool,
    branch: Option<&str>,
    all_branches: bool,
) -> Result<()> {
    let tasks = Plan::resolve(root, daemon_url)?;
    if all_branches {
        let groups = tasks.branch_comments(id)?;
        if json {
            println!("{}", serde_json::to_string_pretty(&groups)?);
            return Ok(());
        }
        for (branch, comment) in merged(&groups) {
            print_comment(Some(branch), comment);
        }
        return Ok(());
    }
    let comments = tasks.comments(id, branch.unwrap_or(tasks.branch()))?;
    if json {
        println!("{}", serde_json::to_string_pretty(&comments)?);
        return Ok(());
    }
    for comment in &comments {
        print_comment(None, comment);
    }
    Ok(())
}

// One stream out of several logs. The earlier timestamp goes first, the branch name breaks a tie,
// and the position within a branch breaks the rest — so the file order inside a branch survives,
// which is the only order a log promises. An entry whose timestamp does not parse takes the one
// before it in its branch, the epoch when it leads, which holds it among the entries it was written
// with.
fn merged(groups: &[op_api::BranchComments]) -> Vec<(&str, &Comment)> {
    let mut all: Vec<(Timestamp, &str, usize, &Comment)> = groups
        .iter()
        .flat_map(|group| {
            let mut carried = Timestamp::default();
            group
                .comments
                .iter()
                .enumerate()
                .map(move |(position, comment)| {
                    carried = comment.at.as_value().map_or(carried, |at| at.0);
                    (carried, group.branch.as_str(), position, comment)
                })
        })
        .collect();
    all.sort_by_key(|(at, branch, position, _)| (*at, *branch, *position));
    all.into_iter()
        .map(|(_, branch, _, comment)| (branch, comment))
        .collect()
}

fn print_comment(branch: Option<&str>, comment: &Comment) {
    let label = match branch {
        Some(branch) => format!("[{branch}] "),
        None => String::new(),
    };
    println!("{label}{}", heading_of(comment));
    for line in comment.text.lines() {
        match line.is_empty() {
            true => println!(),
            false => println!("    {line}"),
        }
    }
    println!();
}

fn heading_of(comment: &Comment) -> String {
    let agent = match &comment.agent {
        Some(agent) => format!(" via {agent}"),
        None => String::new(),
    };
    format!(
        "{} by {}{agent}",
        shown(&comment.at),
        shown(&comment.author)
    )
}

// A field the daemon could not parse reads as the reason it could not, where its value would have
// been: the entry is still worth showing, and hiding why it is broken sends the reader to the file
// with nothing to look for.
fn shown<T: std::fmt::Display>(field: &Field<T>) -> String {
    match field {
        Field::Value(value) => value.to_string(),
        Field::Error(FieldError::Missing) => "(missing)".to_owned(),
        Field::Error(FieldError::Invalid { message }) => format!("({message})"),
    }
}

fn show(root: &Path, daemon_url: Option<&str>, id: &str, branches: bool) -> Result<()> {
    let plan = Plan::resolve(root, daemon_url)?;
    if branches {
        return show_branches(&plan, id);
    }
    let detail = plan.get(id, plan.branch())?;
    let metadata = &detail.metadata;
    println!("id:     {id}");
    println!("title:  {}", detail.title);
    println!("status: {}", status_label(metadata));
    println!("parent: {}", metadata.parent().unwrap_or("-"));
    let dependencies = metadata.dependencies();
    println!(
        "dependencies: {}",
        if dependencies.is_empty() {
            "-".to_owned()
        } else {
            dependencies.join(", ")
        }
    );
    let tags = metadata.tags();
    println!(
        "tags: {}",
        if tags.is_empty() {
            "-".to_owned()
        } else {
            tags.join(", ")
        }
    );
    for problem in metadata.problems() {
        println!("!       {problem}");
    }
    Ok(())
}

fn show_branches(plan: &Plan, id: &str) -> Result<()> {
    let view = plan.branches(id)?;
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
            "  {short}  {:<11} {}",
            status_label(&version.summary.metadata),
            version.summary.title,
        );
        let branches: Vec<String> = version.branches.iter().map(mark_label).collect();
        println!("    branches: {}", branches.join(", "));
    }
    Ok(())
}

fn status_label(metadata: &Metadata) -> String {
    match metadata.status() {
        Some(status) => status.as_str().to_owned(),
        None => "unreadable".to_owned(),
    }
}

fn print_tasks(tasks: &[&TaskListItem]) {
    for task in tasks {
        let status = status_label(&task.metadata);
        println!("{:<10} {status:<11} {}", task.id, task.title);
    }
}

fn print_hits(hits: &[SearchHit]) {
    for hit in hits {
        let status = status_label(&hit.task.metadata);
        println!(
            "{:<22} {:<10} {status:<11} {}",
            hit.branch, hit.task.id, hit.task.title
        );
    }
}

fn cell_flags(cell: &MatrixCell) -> String {
    let mut flags = Vec::new();
    if cell.kind != ChangeKind::Base {
        flags.push(kind_str(cell.kind));
    }
    if cell.dirty {
        flags.push("dirty");
    }
    if flags.is_empty() {
        String::new()
    } else {
        format!("  [{}]", flags.join(", "))
    }
}

fn mark_label(mark: &BranchMark) -> String {
    let mut notes = Vec::new();
    if mark.kind != ChangeKind::Base {
        notes.push(kind_str(mark.kind));
    }
    if mark.dirty {
        notes.push("dirty");
    }
    if notes.is_empty() {
        mark.branch.clone()
    } else {
        format!("{} ({})", mark.branch, notes.join(", "))
    }
}

fn kind_str(kind: ChangeKind) -> &'static str {
    match kind {
        ChangeKind::Base => "base",
        ChangeKind::Added => "added",
        ChangeKind::Modified => "modified",
        ChangeKind::Deleted => "deleted",
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

fn set(root: &Path, daemon_url: Option<&str>, id: &str, field: &str, value: &str) -> Result<()> {
    // Parse before reaching for the daemon so a typo fails without starting one.
    let patch = parse_field(field, value)?;
    Plan::resolve(root, daemon_url)?.patch(id, &patch)?;
    Ok(())
}

fn parse_field(field: &str, value: &str) -> Result<TaskPatch> {
    Ok(match field {
        "status" => TaskPatch {
            status: Some(value.parse()?),
            ..TaskPatch::default()
        },
        // "" or "-" clears the parent (top level), mirroring how `dependencies ""` clears them.
        "parent" => TaskPatch {
            parent: parent_update(parse_parent(value)),
            ..TaskPatch::default()
        },
        "dependencies" => TaskPatch {
            dependencies: Some(comma_separated(value)),
            ..TaskPatch::default()
        },
        // The whole set, like `dependencies`: what the caller names is what the task ends up with,
        // and "" clears it.
        "tags" => TaskPatch {
            tags: Some(tag::identities(comma_separated(value))?),
            ..TaskPatch::default()
        },
        other => bail!("unknown field {other:?}; expected status | parent | dependencies | tags"),
    })
}

fn comma_separated(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

fn parent_update(parent: Option<String>) -> FieldUpdate<String> {
    match parent {
        Some(id) => FieldUpdate::Set(id),
        None => FieldUpdate::Clear,
    }
}

fn delete(root: &Path, daemon_url: Option<&str>, id: &str, yes: bool) -> Result<ExitCode> {
    let plan = Plan::resolve(root, daemon_url)?;
    // The delete targets the caller's branch, so the prompt has to be about a task that branch
    // actually carries — a typo must refuse before it asks the reader to confirm one.
    plan.get(id, plan.branch())?;
    if !yes && !confirm(id)? {
        println!("aborted");
        return Ok(ExitCode::SUCCESS);
    }
    plan.delete(id)?;
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

// The one command that asks about git rather than about plan. Branch names are refs, which every
// worktree of the repository already agrees on; there is no store state to resolve and so nothing
// for the daemon to be the single resolver of.
fn branches(root: &Path) -> Result<()> {
    let repo = Repo::discover(root)?;
    for branch in repo.local_branches()? {
        println!("{branch}");
    }
    Ok(())
}

// The other command that does not ask the daemon. Lint checks the files in front of the caller, as
// a pre-commit hook and a bare checkout need it to — it asks what the bytes on disk say, where every
// task query asks what a branch holds. It writes through the store for the same reason `set` goes
// through the daemon: the advisory lock is what keeps a concurrent writer from seeing a torn file.
fn lint(root: &Path, targets: &[String], json: bool, fix: bool) -> Result<ExitCode> {
    let store = Store::discover(root)?;
    let snapshot = Snapshot::from_store(&store)?;
    let selected = lint_target_paths(&snapshot, &store, targets)?;

    let snapshot = if fix {
        apply_fixes(&store, &snapshot, selected.as_ref())?;
        Snapshot::from_store(&store)?
    } else {
        snapshot
    };

    let diagnostics = op_lint::lint(&snapshot);
    let shown: Vec<&Diagnostic> = diagnostics
        .iter()
        .filter(|d| {
            selected
                .as_ref()
                .is_none_or(|set| set.contains(&canonical(&d.path)))
        })
        .collect();

    if json {
        println!("{}", serde_json::to_string_pretty(&shown)?);
    } else {
        for diagnostic in &shown {
            println!("{diagnostic}");
        }
    }
    Ok(if shown.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

// Writes go through the store so a concurrent daemon or `openplan set` on the same task serializes on
// the advisory lock and never observes a torn file.
fn apply_fixes(
    store: &Store,
    snapshot: &Snapshot,
    selected: Option<&std::collections::HashSet<PathBuf>>,
) -> Result<()> {
    let created = GitCreated::at(store.root());
    let Some(selected) = selected else {
        op_lint::fix_store(store, &created)?;
        return Ok(());
    };
    let fixed = op_lint::fix(snapshot, &created);
    for file in snapshot.files() {
        if !selected.contains(&canonical(&file.path)) {
            continue;
        }
        let Some(after) = fixed.get(&file.path) else {
            continue;
        };
        if after != &file.source {
            store.replace_raw(file.number, after.as_bytes())?;
        }
    }
    for tag in snapshot.tags() {
        if !selected.contains(&canonical(&tag.path)) {
            continue;
        }
        let Some(after) = fixed.get(&tag.path) else {
            continue;
        };
        if after != &tag.source {
            store.replace_raw_tag(&tag.name, after.as_bytes())?;
        }
    }
    Ok(())
}

// The whole store is always scanned; positional targets only pick which files this run reports on
// and repairs, so an agent lints the one task it wrote and a pre-commit hook stays quiet about files
// the commit never touched. None means no filter — report everything.
fn lint_target_paths(
    snapshot: &Snapshot,
    store: &Store,
    targets: &[String],
) -> Result<Option<std::collections::HashSet<PathBuf>>> {
    if targets.is_empty() {
        return Ok(None);
    }
    let mut set = std::collections::HashSet::new();
    for target in targets {
        // A target that resolves to nothing would filter every diagnostic away and pass, so a stale
        // key or a path spelled against the wrong directory has to stop the run instead.
        let Some(path) = lint_target_path(snapshot, store, target) else {
            bail!("no task or tag file matches {target}");
        };
        set.insert(path);
    }
    Ok(Some(set))
}

fn lint_target_path(snapshot: &Snapshot, store: &Store, target: &str) -> Option<PathBuf> {
    if let Some(number) = store.abbreviation().parse_key(target) {
        return snapshot.file(number).map(|file| canonical(&file.path));
    }
    let spellings = [
        canonical(Path::new(target)),
        canonical(&store.root().join(target)),
    ];
    snapshot
        .files()
        .iter()
        .map(|file| &file.path)
        .chain(snapshot.tags().iter().map(|tag| &tag.path))
        .map(|path| canonical(path))
        .find(|path| spellings.contains(path))
}

struct GitCreated {
    repo: Option<Repo>,
    root: PathBuf,
}

impl GitCreated {
    fn at(root: &Path) -> Self {
        GitCreated {
            repo: Repo::discover(root).ok(),
            root: canonical(root),
        }
    }
}

impl CreatedSource for GitCreated {
    fn created(&self, path: &Path) -> Option<Timestamp> {
        // git names blobs by their path from the repo root, and a symlinked or relative `--root`
        // spells the same file differently than the store does, so both sides resolve first.
        let path = canonical(path);
        let relative = path.strip_prefix(&self.root).ok()?;
        self.repo
            .as_ref()?
            .first_commit(relative)
            .ok()
            .flatten()?
            .at
            .ok()
    }
}

// "" or the "-" sentinel clears the parent (top level); any other value sets it.
fn parse_parent(value: &str) -> Option<String> {
    if value.is_empty() || value == "-" {
        None
    } else {
        Some(value.to_owned())
    }
}

fn tree(
    root: &Path,
    daemon_url: Option<&str>,
    id: &str,
    depth: Option<usize>,
    json: bool,
) -> Result<()> {
    let plan = Plan::resolve(root, daemon_url)?;
    let view = plan.tree(id, plan.branch(), depth)?;
    for cycle in &view.cycles {
        eprintln!("warning: parent cycle at {cycle}; its subtree is truncated");
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&view.tree)?);
    } else {
        print_tree(&view.tree, 0);
    }
    Ok(())
}

fn print_tree(node: &TaskTree, depth: usize) {
    println!(
        "{}{:<10} {:<11} {}",
        "  ".repeat(depth),
        node.id,
        status_label(&node.metadata),
        node.title,
    );
    for child in &node.children {
        print_tree(child, depth + 1);
    }
}

fn move_task(
    root: &Path,
    daemon_url: Option<&str>,
    id: &str,
    parent: Option<String>,
    before: Option<String>,
    after: Option<String>,
) -> Result<()> {
    // The ranks are computed from the same state the write lands on: the daemon's view of the
    // caller's branch, not a second reading of the files.
    let plan = Plan::resolve(root, daemon_url)?;
    let group = plan.list(plan.branch())?;
    // The task's own row, from the same read the siblings come from: asking for it separately would
    // walk the repository a second time to learn what this list already says.
    let current_parent = group
        .iter()
        .find(|task| task.id == id)
        .ok_or_else(|| anyhow::anyhow!("no such task: {id}"))?
        .metadata
        .parent()
        .map(str::to_owned);
    let new_parent = match parent {
        None => current_parent,
        Some(value) => parse_parent(&value),
    };
    let mut siblings: Vec<&TaskListItem> = group
        .iter()
        .filter(|s| s.metadata.parent() == new_parent.as_deref() && s.id != id)
        .collect();
    siblings.sort_by(|a, b| list_item_cmp(a, b));
    let insert = insert_index(&siblings, before.as_deref(), after.as_deref())?;

    match rank_plan(&siblings, insert) {
        RankPlan::Single(new_rank) => {
            plan.patch(
                id,
                &TaskPatch {
                    parent: parent_update(new_parent),
                    rank: Some(new_rank),
                    ..TaskPatch::default()
                },
            )?;
        }
        // A sibling group with missing, colliding, or malformed ranks can't be split by a single
        // fractional key, so materialize a fresh, evenly-spaced order for the whole group.
        RankPlan::Rebalance {
            siblings: assigned,
            x_rank,
        } => {
            // The moved task goes first: its write is the one the store validates (parent exists,
            // no cycle), so a refused move leaves the siblings' ranks untouched.
            plan.patch(
                id,
                &TaskPatch {
                    parent: parent_update(new_parent),
                    rank: Some(x_rank),
                    ..TaskPatch::default()
                },
            )?;
            for (sibling_id, sibling_rank) in assigned {
                plan.patch(
                    &sibling_id,
                    &TaskPatch {
                        rank: Some(sibling_rank),
                        ..TaskPatch::default()
                    },
                )?;
            }
        }
    }
    Ok(())
}

fn insert_index(
    siblings: &[&TaskListItem],
    before: Option<&str>,
    after: Option<&str>,
) -> Result<usize> {
    if let Some(before) = before {
        let pos = sibling_pos(siblings, before)?;
        Ok(pos)
    } else if let Some(after) = after {
        let pos = sibling_pos(siblings, after)?;
        Ok(pos + 1)
    } else {
        Ok(siblings.len())
    }
}

fn sibling_pos(siblings: &[&TaskListItem], target: &str) -> Result<usize> {
    siblings
        .iter()
        .position(|s| s.id == target)
        .ok_or_else(|| anyhow::anyhow!("{target} is not a sibling under the target parent"))
}

enum RankPlan {
    Single(String),
    Rebalance {
        siblings: Vec<(String, String)>,
        x_rank: String,
    },
}

fn rank_plan(siblings: &[&TaskListItem], insert: usize) -> RankPlan {
    let ranks: Vec<&str> = siblings.iter().filter_map(|s| s.metadata.rank()).collect();
    if ranks.len() == siblings.len() && rank::is_ordered(&ranks) {
        let neighbour = |i: usize| ranks.get(i).copied();
        let between = rank::between(insert.checked_sub(1).and_then(neighbour), neighbour(insert));
        if let Some(new_rank) = between {
            return RankPlan::Single(new_rank);
        }
    }
    let keys = rank::spaced(siblings.len() + 1);
    let assigned = siblings
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let slot = if i < insert { i } else { i + 1 };
            (s.id.clone(), keys[slot].clone())
        })
        .collect();
    RankPlan::Rebalance {
        siblings: assigned,
        x_rank: keys[insert].clone(),
    }
}
