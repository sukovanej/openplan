mod author;
mod daemon;
mod doc;
mod history;
mod lint;
mod open;
mod plan;
mod project;
mod start;
mod tag;
mod tasks;
mod update;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context as _, Result, bail};
use clap::builder::{PossibleValuesParser, TypedValueParser};
use clap::{Parser, Subcommand, ValueEnum};
use op_api::BackendKind;
use op_task::Status;
use op_task::tag::Color;

use op_daemon::{Home, Updates};
use op_update::Channel;
use plan::Plan;
use serde::Serialize;

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
    /// Manage the tasks of this project
    Tasks {
        #[command(subcommand)]
        command: TaskCommand,
    },
    /// Print the revisions of the project, or of one task, newest first, and what each one changed
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
    /// Print the web UI address of each task (by key) or doc (by name)
    Url {
        #[arg(required = true)]
        keys: Vec<String>,
        #[arg(long)]
        json: bool,
    },
    /// Report problems in the tasks and docs
    Lint {
        /// Report only these tasks; every task is checked all the same
        keys: Vec<String>,
        #[arg(long)]
        json: bool,
    },
    /// Manage the project documents
    Doc {
        #[command(subcommand)]
        command: DocCommand,
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
    Update {
        /// Install the newest canary build of `main` instead. A later `openplan update` goes
        /// back to the stable release
        #[arg(long)]
        canary: bool,
        /// Turn the daemon's own updates on or off, and install nothing now. The daemon checks
        /// every hour and installs a new release when no agent session runs
        #[arg(long, value_enum, conflicts_with = "canary")]
        auto: Option<Toggle>,
    },
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
enum Toggle {
    On,
    Off,
}

#[derive(Subcommand)]
enum TaskCommand {
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
        /// Link a pull request or merge request, by its address or by its number in the
        /// repository of this project; repeat for more
        #[arg(long = "pr")]
        pull_requests: Vec<String>,
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
    /// Replace a whole task with a task file, as `openplan tasks get` prints one
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
    /// Print a task's metadata (status, parent, dependencies, tags, pull requests)
    Show {
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Link and unlink the pull requests and merge requests of a task
    Pr {
        #[command(subcommand)]
        command: PullRequestCommand,
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
    /// Set a validated field: status | parent | dependencies | tags | pull_requests
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
}

#[derive(Subcommand)]
enum PullRequestCommand {
    /// Link a pull request to a task
    Add {
        id: String,
        /// The address, or the number in the repository of this project
        pull_request: String,
    },
    /// Unlink a pull request from a task
    Remove {
        id: String,
        /// The address, or the number in the repository of this project
        pull_request: String,
    },
}

#[derive(Subcommand)]
enum DocCommand {
    /// Write a new doc and print the name it normalizes to
    Create {
        name: String,
        /// Markdown content placed below the title heading
        #[arg(long)]
        body: Option<String>,
        /// Name of the doc to nest this one under
        #[arg(long)]
        parent: Option<String>,
    },
    /// List the docs of this project
    List {
        #[arg(long)]
        json: bool,
    },
    /// Print a doc as markdown, or its metadata as JSON
    Get {
        name: String,
        #[arg(long)]
        json: bool,
    },
    /// Replace the markdown below the title heading
    Set { name: String, body: String },
    /// Nest a doc under another one; "" or "-" moves it to the top level
    Nest { name: String, parent: String },
    /// Rename a doc; its title heading and the docs nested under it follow the name
    Rename { from: String, to: String },
    /// Delete a doc
    Delete {
        name: String,
        #[arg(long)]
        yes: bool,
    },
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
    Colors {
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum ProjectCommand {
    /// List every registered project, with the reason a demoted one is not served
    List {
        #[arg(long)]
        json: bool,
    },
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
    Ping {
        #[arg(long)]
        json: bool,
    },
    /// Print the HTTP API's OpenAPI 3.1 spec to stdout
    Openapi,
}

fn main() -> ExitCode {
    if let Some(code) = op_daemon::serve_if_requested(std::env::args(), Updates::Auto) {
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
        Command::Tasks { command } => {
            tasks::run(command, root, daemon_url).map(|()| ExitCode::SUCCESS)
        }
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
        Command::Url { keys, json } => {
            page_urls(root, daemon_url, &keys, json).map(|()| ExitCode::SUCCESS)
        }
        Command::Lint { keys, json } => lint::run(root, &keys, json),
        Command::Doc { command } => doc::run(command, root, daemon_url).map(|()| ExitCode::SUCCESS),
        Command::Tag { command } => tag::run(command, root, daemon_url).map(|()| ExitCode::SUCCESS),
        Command::Project { command } => {
            project::run(command, root, daemon_url).map(|()| ExitCode::SUCCESS)
        }
        Command::Update {
            auto: Some(toggle), ..
        } => update::auto(matches!(toggle, Toggle::On)).map(|()| ExitCode::SUCCESS),
        Command::Update { canary, auto: None } => {
            let channel = if canary {
                Channel::Canary
            } else {
                Channel::Stable
            };
            update::run(channel).map(|()| ExitCode::SUCCESS)
        }
        Command::Server { command } => server(command, daemon_url),
    }
}

fn server(command: ServerCommand, daemon_url: Option<&str>) -> Result<ExitCode> {
    match command {
        ServerCommand::Start { port, foreground } => {
            reject_remote_override(daemon_url, "start")?;
            if foreground {
                return Ok(op_daemon::serve(Home::resolve()?, port, Updates::Auto));
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
        ServerCommand::Ping { json } => {
            let running = daemon::ping(daemon_url, json)?;
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

#[derive(Serialize)]
struct PageUrl<'a> {
    key: &'a str,
    url: String,
}

// A link to a task or a doc that does not exist opens an empty page, so a mistyped one fails here.
fn page_urls(root: &Path, daemon_url: Option<&str>, keys: &[String], json: bool) -> Result<()> {
    let plan = Plan::resolve(root, daemon_url)?;
    let urls = keys
        .iter()
        .map(|key| {
            let url = if op_task::is_key_shaped(key) {
                plan.get(key)?;
                plan.task_page(key)
            } else {
                let name = doc::identity(key)?;
                plan.doc(&name)?;
                plan.doc_page(&name)
            };
            Ok(PageUrl { key, url })
        })
        .collect::<Result<Vec<_>>>()?;
    if json {
        println!("{}", serde_json::to_string_pretty(&urls)?);
    } else {
        for page in &urls {
            println!("{}", page.url);
        }
    }
    Ok(())
}
