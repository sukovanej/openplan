mod author;
mod daemon;
mod history;
mod lint;
mod open;
mod plan;
mod project;
mod start;
mod tag;
mod update;

use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context as _, Result, bail};
use clap::builder::{PossibleValuesParser, TypedValueParser};
use clap::{Parser, Subcommand, ValueEnum};
use op_api::{
    BackendKind, Comment, CreateComment, CreateTask, Field, FieldError, FieldUpdate, Metadata,
    SearchHit, TaskListItem, TaskPatch, TaskTree, list_item_cmp,
};
use op_task::tag::Color;
use op_task::{Status, rank};

use op_daemon::Home;
use plan::Plan;

#[derive(Parser)]
#[command(name = "openplan", version, about = "openplan — local-first task CLI")]
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
    /// Start the tasks of this project, or join the tasks its git remote holds
    Init {
        /// Where the tasks live; omit for git in a repository and local elsewhere
        #[arg(long, value_enum)]
        backend: Option<Backend>,
        /// The three uppercase letters every task key starts with, like OPP in OPP-42; omit to
        /// join the tasks on the git remote
        #[arg(long)]
        abbreviation: Option<String>,
    },
    /// Move the tasks of a repository that keeps them in .plan/ beside the code
    Migrate {
        /// Where the tasks go; omit for the git ref refs/openplan/tasks with the history of .plan/
        #[arg(long, value_enum)]
        backend: Option<Backend>,
    },
    /// Install or update the OpenPlan agent skills in this repository
    SetupSkills {
        /// Install skills for one agent; omit to install for all agents
        #[arg(long, value_enum)]
        agent: Option<Agent>,
    },
    /// Create a task and print its new id
    Create {
        title: String,
        #[arg(long)]
        parent: Option<String>,
        #[arg(long, value_parser = status_parser())]
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
    /// List the tasks of this project
    List {
        #[arg(long, value_parser = status_parser())]
        status: Option<Status>,
        #[arg(long)]
        parent: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Find tasks whose title, body, or frontmatter contains the query
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
        /// Print the task as it stood at this revision, as `openplan history` names it
        #[arg(long)]
        revision: Option<String>,
    },
    /// Replace a whole task with a task file, as `openplan get` prints one
    Write {
        id: String,
        /// Read the file from this path, or `-` for stdin
        #[arg(long)]
        file: String,
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
    },
    /// Print a task's metadata (status, parent, dependencies, tags)
    Show { id: String },
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
    /// Delete a task
    Delete {
        id: String,
        #[arg(long)]
        yes: bool,
    },
    /// Print the revisions of the project, or of one task, newest first
    History {
        /// A task key; omit for every revision of the project
        id: Option<String>,
        /// Only revisions older than this one
        #[arg(long)]
        before: Option<String>,
        #[arg(long, default_value_t = 20)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    /// Exchange the tasks with the remote now, rather than at the next automatic sync
    Sync {
        /// Print how the last sync went, and sync nothing
        #[arg(long)]
        status: bool,
        #[arg(long)]
        json: bool,
    },
    /// Open the realtime web UI in the default browser
    Open,
    /// Report task problems (fields, references, cycles, tags, conflicts) and stale agent skills; never starts a daemon
    Lint {
        /// Report only these tasks; every task is checked all the same
        keys: Vec<String>,
        #[arg(long)]
        json: bool,
        /// Check only the agent skill files of this checkout, not the tasks
        #[arg(long, conflicts_with = "keys")]
        skills: bool,
    },
    /// Manage the tags tasks can carry
    Tag {
        #[command(subcommand)]
        command: TagCommand,
    },
    /// Manage the repositories the daemon serves
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
    /// Replace this CLI and the desktop app with the newest release
    Update,
    /// Manage the background daemon and web UI
    Server {
        #[command(subcommand)]
        command: ServerCommand,
    },
}

fn status_parser() -> impl TypedValueParser<Value = Status> {
    PossibleValuesParser::new(Status::ALL.map(|s| s.as_str())).try_map(|s| s.parse::<Status>())
}

fn color_parser() -> impl TypedValueParser<Value = Color> {
    PossibleValuesParser::new(Color::ALL.map(|c| c.as_str())).try_map(|c| c.parse::<Color>())
}

#[derive(Clone, Copy, ValueEnum)]
enum Backend {
    Git,
    Local,
}

impl Backend {
    fn kind(self) -> BackendKind {
        match self {
            Self::Git => BackendKind::Git,
            Self::Local => BackendKind::Local,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum Agent {
    Claude,
    Codex,
}

#[derive(Subcommand)]
enum TagCommand {
    /// Register a tag and print the name it normalizes to
    Create {
        name: String,
        #[arg(long, value_parser = color_parser())]
        color: Option<Color>,
        #[arg(long = "desc")]
        description: Option<String>,
    },
    /// List every tag the project registers
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
    /// Rename a tag and rewrite the tasks that carry the old name
    Rename { from: String, to: String },
    /// Delete a tag file
    Delete {
        name: String,
        /// Delete the tag even while tasks carry it; each of those tasks keeps a name the project
        /// does not register, and refuses every write until the name goes
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
    /// Drop a project from the registry; its tasks stay where they are
    Remove { name: String },
    /// Give a project a new name; its URLs change with it
    Rename { from: String, to: String },
}

#[derive(Subcommand)]
enum ServerCommand {
    /// Start the daemon detached; a no-op if one is already running
    Start {
        #[arg(long, env = "OPENPLAN_PORT", default_value_t = op_daemon::DEFAULT_PORT)]
        port: u16,
        /// Run in this terminal instead of detaching
        #[arg(long)]
        foreground: bool,
    },
    /// Stop the running daemon, gracefully if it answers
    Stop,
    /// Stop the running daemon, then start a fresh one
    Restart {
        #[arg(long, env = "OPENPLAN_PORT", default_value_t = op_daemon::DEFAULT_PORT)]
        port: u16,
    },
    /// Report daemon status without starting it
    Ping,
    /// Print the HTTP API's OpenAPI 3.1 spec to stdout
    Openapi,
}

fn main() -> ExitCode {
    if let Some(code) = op_daemon::serve_if_requested(std::env::args()) {
        return code;
    }
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
    // Every command works in the current directory unless told otherwise. The tasks can live in an
    // ancestor of it, and a relative path has no ancestors to search.
    let root = std::path::absolute(cli.root.as_deref().unwrap_or_else(|| Path::new(".")))
        .context("resolve the directory to work in")?;
    let root = root.as_path();
    match cli.command {
        Command::Init {
            backend,
            abbreviation,
        } => start::init(
            root,
            daemon_url,
            backend.map(Backend::kind),
            abbreviation.as_deref(),
        )
        .map(|()| ExitCode::SUCCESS),
        Command::Migrate { backend } => {
            start::migrate(root, daemon_url, backend.map(Backend::kind)).map(|()| ExitCode::SUCCESS)
        }
        Command::SetupSkills { agent } => {
            let agents = match agent {
                Some(Agent::Claude) => vec![op_skills::Agent::Claude],
                Some(Agent::Codex) => vec![op_skills::Agent::Codex],
                None => op_skills::Agent::ALL.to_vec(),
            };
            op_skills::setup(&lint::skills_root(root), &agents)?;
            Ok(ExitCode::SUCCESS)
        }
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
        } => list(root, daemon_url, status, parent.as_deref(), json).map(|()| ExitCode::SUCCESS),
        Command::Search { query, json } => {
            search(root, daemon_url, &query, json).map(|()| ExitCode::SUCCESS)
        }
        Command::Get { id, json, revision } => {
            get(root, daemon_url, &id, json, revision.as_deref()).map(|()| ExitCode::SUCCESS)
        }
        Command::Write { id, file } => {
            let text = resolve_body(None, Some(file))?.unwrap_or_default();
            write(root, daemon_url, &id, &text).map(|()| ExitCode::SUCCESS)
        }
        Command::Comment {
            id,
            text,
            body_file,
        } => {
            let text = resolve_body(text, body_file)?.unwrap_or_default();
            comment(root, daemon_url, &id, &text).map(|()| ExitCode::SUCCESS)
        }
        Command::Comments { id, json } => {
            comments(root, daemon_url, &id, json).map(|()| ExitCode::SUCCESS)
        }
        Command::Show { id } => show(root, daemon_url, &id).map(|()| ExitCode::SUCCESS),
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
        Command::History {
            id,
            before,
            limit,
            json,
        } => history::history(
            root,
            daemon_url,
            id.as_deref(),
            before.as_deref(),
            limit,
            json,
        )
        .map(|()| ExitCode::SUCCESS),
        Command::Sync { status, json } => history::sync(root, daemon_url, status, json),
        Command::Open => open::run(root, daemon_url).map(|()| ExitCode::SUCCESS),
        Command::Lint { keys, json, skills } => lint::run(root, &keys, json, skills),
        Command::Tag { command } => tag::run(command, root, daemon_url).map(|()| ExitCode::SUCCESS),
        Command::Project { command } => {
            project::run(command, root, daemon_url).map(|()| ExitCode::SUCCESS)
        }
        Command::Update => update::run().map(|()| ExitCode::SUCCESS),
        Command::Server { command } => server(command, daemon_url),
    }
}

fn server(command: ServerCommand, daemon_url: Option<&str>) -> Result<ExitCode> {
    match command {
        ServerCommand::Start { port, foreground } => {
            reject_remote_override(daemon_url, "start")?;
            if foreground {
                return Ok(op_daemon::serve(Home::resolve()?, port));
            }
            daemon::start(port)?;
            Ok(ExitCode::SUCCESS)
        }
        ServerCommand::Stop => {
            daemon::stop(daemon_url)?;
            Ok(ExitCode::SUCCESS)
        }
        ServerCommand::Restart { port } => {
            reject_remote_override(daemon_url, "restart")?;
            daemon::restart(port)?;
            Ok(ExitCode::SUCCESS)
        }
        ServerCommand::Ping => {
            let running = daemon::ping(daemon_url)?;
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
) -> Result<()> {
    let held = Plan::resolve(root, daemon_url)?.list()?;
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
    revision: Option<&str>,
) -> Result<()> {
    let plan = Plan::resolve(root, daemon_url)?;
    if let Some(revision) = revision {
        let then = plan.revision(id, revision)?;
        if json {
            println!("{}", serde_json::to_string_pretty(&then)?);
            return Ok(());
        }
        match then.task {
            Some(task) => print!("{}", task.raw),
            None => bail!("{id} did not exist at revision {revision}"),
        }
        return Ok(());
    }
    let detail = plan.get(id)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&detail)?);
    } else {
        // The daemon holds parsed state, so this is a canonical rendering of the task and not a copy
        // of its file. A field it could not parse has no canonical form, so it is reported instead.
        for problem in &detail.problems {
            eprintln!("{id}: {}", problem.message);
        }
        if detail.conflicts > 0 {
            eprintln!("{}", conflict_notice(id, detail.conflicts));
        }
        print!(
            "{}",
            op_api::render_task_file(&detail.metadata, &detail.body, &detail.comments)?
        );
    }
    Ok(())
}

fn write(root: &Path, daemon_url: Option<&str>, id: &str, text: &str) -> Result<()> {
    let detail = Plan::resolve(root, daemon_url)?.write(id, text)?;
    println!("{}: {}", detail.id, detail.title);
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

fn comments(root: &Path, daemon_url: Option<&str>, id: &str, json: bool) -> Result<()> {
    let comments = Plan::resolve(root, daemon_url)?.comments(id)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&comments)?);
        return Ok(());
    }
    for comment in &comments {
        print_comment(comment);
    }
    Ok(())
}

fn print_comment(comment: &Comment) {
    println!("{}", heading_of(comment));
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
        Field::Conflict(conflict) => conflict.value.to_string(),
        Field::Error(FieldError::Missing) => "(missing)".to_owned(),
        Field::Error(FieldError::Invalid { message }) => format!("({message})"),
    }
}

fn show(root: &Path, daemon_url: Option<&str>, id: &str) -> Result<()> {
    let detail = Plan::resolve(root, daemon_url)?.get(id)?;
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
    for problem in &detail.problems {
        println!("!       {}", problem.message);
    }
    if detail.conflicts > 0 {
        let fields = metadata.conflicted_fields();
        match fields.is_empty() {
            true => println!("conflicts: {}", detail.conflicts),
            false => println!(
                "conflicts: {} (fields: {})",
                detail.conflicts,
                fields.join(", ")
            ),
        }
        eprintln!("{}", conflict_notice(id, detail.conflicts));
    }
    Ok(())
}

// Sync leaves a conflict where two people changed one thing differently; the agent or person who
// reads the task settles it.
fn conflict_notice(id: &str, count: usize) -> String {
    format!(
        "{id} has {count} unresolved conflict{} from a sync. Each block between `<<<<<<<` and \
         `>>>>>>>` holds two versions, and the one after `=======` is in force. Keep the right \
         version of each block, remove the markers, and write the task back with `openplan write`, \
         or settle one field with `openplan set`.",
        if count == 1 { "" } else { "s" }
    )
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
        let conflicted = match task.conflicts {
            0 => "",
            _ => "  [conflict]",
        };
        let problems = match task.problems.len() {
            0 => String::new(),
            1 => "  [1 problem]".to_owned(),
            count => format!("  [{count} problems]"),
        };
        println!(
            "{:<10} {status:<11} {}{conflicted}{problems}",
            task.id, task.title
        );
    }
}

fn print_hits(hits: &[SearchHit]) {
    for hit in hits {
        let status = status_label(&hit.task.metadata);
        println!("{:<10} {status:<11} {}", hit.task.id, hit.task.title);
    }
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
    // A typo must refuse before it asks the reader to confirm a delete.
    plan.get(id)?;
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
    let view = Plan::resolve(root, daemon_url)?.tree(id, depth)?;
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
    // The ranks are computed from the daemon's view of the tasks, the state the write lands on.
    let plan = Plan::resolve(root, daemon_url)?;
    let group = plan.list()?;
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
            // The moved task goes first: its write is the one the daemon validates (parent exists,
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
