mod daemon;
mod mergedriver;
mod project;
mod serve;
mod writer;

use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context as _, Result, bail};
use clap::{Parser, Subcommand};
use op_api::{
    BranchMark, ChangeKind, CreateTask, FieldUpdate, KeyError, Matrix, MatrixCell, Metadata,
    TaskPatch, TaskSummary, TaskTree, TaskView, sibling_cmp,
};
use op_git::Repo;
use op_index::Index;
use op_lint::{CreatedSource, Diagnostic, Snapshot};
use op_store::Store;
use op_task::{FieldError, FieldResult, Status, Timestamp, rank};

use daemon::{Control, Home};
use writer::Writer;

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
    /// Print a task's metadata (status, parent, dependencies)
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
    /// Set a validated field: status | parent | dependencies
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
            body,
            body_file,
        } => {
            let body = resolve_body(body, body_file)?;
            create(root, daemon_url, title, parent, status, dependencies, body)
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
            status,
            parent.as_deref(),
            json,
            all_branches,
            branch.as_deref(),
        )
        .map(|()| ExitCode::SUCCESS),
        Command::Get { id, json, branch } => {
            get(root, &id, json, branch.as_deref()).map(|()| ExitCode::SUCCESS)
        }
        Command::Show { id, branches } => show(root, &id, branches).map(|()| ExitCode::SUCCESS),
        Command::Tree { id, depth, json } => {
            tree(root, &id, depth, json).map(|()| ExitCode::SUCCESS)
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
        Command::Lint { targets, json, fix } => lint(root, &targets, json, fix),
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

fn create(
    root: &Path,
    daemon_url: Option<&str>,
    title: String,
    parent: Option<String>,
    status: Option<Status>,
    dependencies: Vec<String>,
    body: Option<String>,
) -> Result<()> {
    let id = Writer::resolve(root, daemon_url)?.create(&CreateTask {
        title,
        status,
        parent,
        dependencies,
        body,
    })?;
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
    for number in &ids {
        let key = store.abbreviation().format_key(*number);
        match store.read_raw(*number) {
            Ok(raw) => {
                let summary = TaskSummary::from_partial(
                    key,
                    op_task::parse_partial(&raw),
                    store.abbreviation(),
                );
                if status.is_some_and(|s| summary.metadata.status() != Some(s)) {
                    continue;
                }
                if parent.is_some_and(|p| summary.metadata.parent() != Some(p)) {
                    continue;
                }
                summaries.push(summary);
            }
            // stderr keeps the diagnostic out of stdout's JSON while still surfacing it.
            Err(err) => eprintln!("{key}: {err}"),
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
        .filter(|cell| status.is_none_or(|s| cell.task.metadata.status() == Some(s)))
        .cloned()
        .collect();
    if json {
        println!("{}", serde_json::to_string_pretty(&Matrix { cells })?);
    } else if cells.is_empty() {
        println!("no tasks on any branch");
    } else {
        for cell in &cells {
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
        .filter(|s| status.is_none_or(|st| s.metadata.status() == Some(st)))
        .filter(|s| parent.is_none_or(|p| s.metadata.parent() == Some(p)))
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
    let number = number_of(&store, id)?;
    if json {
        let partial = op_task::parse_partial(&store.read_raw(number)?);
        let view = TaskView::from_partial(
            id.to_owned(),
            partial,
            local_updated(root, id),
            store.abbreviation(),
        );
        println!("{}", serde_json::to_string_pretty(&view)?);
    } else {
        // Print the file verbatim; re-serializing would normalize formatting and is a lossy
        // view of what is actually on disk.
        print!("{}", store.read_raw(number)?);
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
    let partial = op_task::parse_partial(&store.read_raw(number_of(&store, id)?)?);
    let summary = TaskSummary::from_partial(id.to_owned(), partial, store.abbreviation());
    let metadata = &summary.metadata;
    println!("id:     {id}");
    println!("title:  {}", summary.title);
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
    for problem in metadata.problems() {
        println!("!       {problem}");
    }
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
            "  {short}  {:<11} {}",
            status_label(&version.summary.metadata),
            version.summary.title,
        );
        let branches: Vec<String> = version.branches.iter().map(mark_label).collect();
        println!("    branches: {}", branches.join(", "));
    }
    Ok(())
}

// `updated` is git-derived, so a store that sits outside a repository — or a task no commit holds —
// simply has none to report. It dates the checked-out branch, the one whose file was just read; a
// task matching its merge-base has no cell of its own there, and falls back to the branch that did
// last change it.
//
// A running daemon already holds this, kept warm across requests. Asking it spares a one-shot
// process the whole cross-branch index — every branch's blobs parsed and its history walked — built
// to fill this one field and then dropped.
fn local_updated(root: &Path, id: &str) -> FieldResult<Timestamp> {
    let (Ok(repo), Ok(store)) = (Repo::discover(root), Store::discover(root)) else {
        return Err(FieldError::Missing);
    };
    // The worktree's own root, not the `--root` the caller passed, which may be any directory
    // beneath it.
    let branch = repo.worktree_branch(store.root()).ok().flatten();
    if let Some(updated) = Control::resolve()
        .ok()
        .and_then(|control| control.task_updated(&repo.git_common_dir(), id, branch.as_deref()))
    {
        return updated;
    }
    let Ok((_repo, index)) = build_index(root) else {
        return Err(FieldError::Missing);
    };
    index.task_updated_or_headline(id, index.current_branch())
}

fn build_index(root: &Path) -> Result<(Repo, Index)> {
    let repo = Repo::discover(root)?;
    let store = Store::discover(root)?;
    let mut index = Index::new(store.abbreviation());
    index.rebuild(&repo, &store)?;
    Ok((repo, index))
}

// Every id a command takes or prints is a key; the number behind it goes no further than the
// store call it was resolved for.
fn number_of(store: &Store, key: &str) -> Result<u64> {
    store
        .abbreviation()
        .parse_key(key)
        .ok_or_else(|| KeyError::new(store.abbreviation(), key).into())
}

fn ensure_branch(repo: &Repo, branch: &str) -> Result<()> {
    if repo.local_branches()?.iter().any(|b| b == branch) {
        Ok(())
    } else {
        bail!("no such branch: {branch}");
    }
}

fn status_label(metadata: &Metadata) -> String {
    match metadata.status() {
        Some(status) => status.as_str().to_owned(),
        None => "unreadable".to_owned(),
    }
}

fn print_summaries(summaries: &[TaskSummary]) {
    for summary in summaries {
        let status = status_label(&summary.metadata);
        println!("{:<10} {status:<11} {}", summary.id, summary.title);
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
    Writer::resolve(root, daemon_url)?.patch(id, &patch)?;
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
            dependencies: Some(
                value
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .collect(),
            ),
            ..TaskPatch::default()
        },
        other => bail!("unknown field {other:?}; expected status | parent | dependencies"),
    })
}

fn parent_update(parent: Option<String>) -> FieldUpdate<String> {
    match parent {
        Some(id) => FieldUpdate::Set(id),
        None => FieldUpdate::Clear,
    }
}

fn delete(root: &Path, daemon_url: Option<&str>, id: &str, yes: bool) -> Result<ExitCode> {
    // A local read answers this: the delete targets the caller's branch, whose worktree is this one.
    // Prompting — and starting a daemon — for a typo would be the daemon's 404 arriving too late.
    let store = Store::discover(root)?;
    if !store.exists(number_of(&store, id)?) {
        bail!("no such task: {id}");
    }
    if !yes && !confirm(id)? {
        println!("aborted");
        return Ok(ExitCode::SUCCESS);
    }
    Writer::resolve(root, daemon_url)?.delete(id)?;
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

fn branches(root: &Path) -> Result<()> {
    let repo = Repo::discover(root)?;
    for branch in repo.local_branches()? {
        println!("{branch}");
    }
    Ok(())
}

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
            bail!("no task file matches {target}");
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
        .map(|file| canonical(&file.path))
        .find(|path| spellings.contains(path))
}

fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
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

fn local_summaries(store: &Store) -> Result<Vec<TaskSummary>> {
    let mut summaries = Vec::new();
    for number in store.task_ids()? {
        let key = store.abbreviation().format_key(number);
        match store.read_raw(number) {
            Ok(raw) => summaries.push(TaskSummary::from_partial(
                key,
                op_task::parse_partial(&raw),
                store.abbreviation(),
            )),
            Err(err) => eprintln!("{key}: {err}"),
        }
    }
    Ok(summaries)
}

fn tree(root: &Path, id: &str, depth: Option<usize>, json: bool) -> Result<()> {
    let store = Store::discover(root)?;
    if !store.exists(number_of(&store, id)?) {
        bail!("no such task: {id}");
    }
    let summaries = local_summaries(&store)?;
    let mut cycles = Vec::new();
    let tree = TaskTree::build(&summaries, id, depth, &mut cycles)
        .ok_or_else(|| anyhow::anyhow!("no such task: {id}"))?;
    let mut reported = std::collections::BTreeSet::new();
    for cycle in cycles {
        if reported.insert(cycle.clone()) {
            eprintln!("warning: parent cycle at {cycle}; its subtree is truncated");
        }
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&tree)?);
    } else {
        print_tree(&tree, 0);
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
    // The sibling group is read locally — reads are global, writes go through the daemon.
    let store = Store::discover(root)?;
    let current_parent = store
        .read(number_of(&store, id)?)?
        .frontmatter
        .parent
        .as_deref()
        .and_then(|parent| store.abbreviation().format_ref(parent));
    let new_parent = match parent {
        None => current_parent,
        Some(value) => parse_parent(&value),
    };
    let summaries = local_summaries(&store)?;
    let mut siblings: Vec<&TaskSummary> = summaries
        .iter()
        .filter(|s| s.metadata.parent() == new_parent.as_deref() && s.id != id)
        .collect();
    siblings.sort_by(|a, b| sibling_cmp(a, b));
    let insert = insert_index(&siblings, before.as_deref(), after.as_deref())?;

    let writer = Writer::resolve(root, daemon_url)?;
    match rank_plan(&siblings, insert) {
        RankPlan::Single(new_rank) => {
            writer.patch(
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
            writer.patch(
                id,
                &TaskPatch {
                    parent: parent_update(new_parent),
                    rank: Some(x_rank),
                    ..TaskPatch::default()
                },
            )?;
            for (sibling_id, sibling_rank) in assigned {
                writer.patch(
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
    siblings: &[&TaskSummary],
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

fn sibling_pos(siblings: &[&TaskSummary], target: &str) -> Result<usize> {
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

fn rank_plan(siblings: &[&TaskSummary], insert: usize) -> RankPlan {
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
