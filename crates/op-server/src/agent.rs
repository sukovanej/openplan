use std::collections::BTreeMap;
use std::convert::Infallible;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use op_agent::{
    Agent, AgentError, AgentEvent, AgentKind, ApprovalDecision, ApprovalId, Entry, McpPolicy,
    Permissions, Persistence, Session, SessionHandle, SessionId, SessionOptions, Status,
    ToolOutcome, Transcript,
};
use op_agent_claude::{ClaudeCode, Skills, Tools};
use op_agent_codex::Codex;
use op_api::{ApiErrorBody, ChangeEvent, Rfc3339};
use op_task::{Abbreviation, Timestamp};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::{Mutex as AsyncMutex, broadcast, mpsc, watch};
use tokio::task::JoinHandle;
use tokio_stream::Stream;
use tokio_stream::wrappers::ReceiverStream;
use utoipa::ToSchema;

use crate::agent_store::{AgentStore, StoredSession};
use crate::tasks::no_such_task;
use crate::{ApiError, AppState, EVENT_CHANNEL_CAPACITY, Project, Publisher, blocking, project_of};

const SESSION_ID_BYTES: usize = 8;
// The driver kills a child that ignores a closed stdin after five seconds, so a stop that waits
// past that mark sees every session out rather than cutting one short.
const STOP_GRACE: Duration = Duration::from_secs(6);
const TOOLS: [&str; 6] = ["Read", "Grep", "Glob", "Bash", "Edit", "Write"];
// The alias the `claude` binary resolves to its current Sonnet: a task is a page of prose, and
// Sonnet writes one for a fraction of what Opus asks.
const CLAUDE_MODEL: &str = "sonnet";
const TASK_WRITES: [&str; 6] = ["create", "write", "set", "move", "delete", "comment"];

// Where the agent works: the project's checkout, which it reads to design a task. It writes tasks
// through the CLI and never a file.
#[derive(Debug, Clone)]
pub struct Workspace {
    pub cwd: PathBuf,
}

impl Workspace {
    pub fn of(project: &Project) -> Self {
        Self {
            cwd: project.path.clone(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateSession {
    #[schema(value_type = Option<String>, example = "claude_code")]
    #[serde(default)]
    pub agent: Option<AgentKind>,
    #[serde(default)]
    pub context: Option<String>,
    pub prompt: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CreatedSession {
    pub id: String,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct Say {
    pub text: String,
}

// `ApprovalDecision` belongs to a crate that knows nothing of HTTP; this gives utoipa the schema
// without changing the bytes on the wire.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(transparent)]
#[schema(value_type = Object)]
pub struct Decision(pub ApprovalDecision);

// `context` is the task that was open when the session started; `tasks` are the tasks it wrote, in
// the order it first wrote them.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SessionSummary {
    pub id: String,
    pub project: String,
    #[schema(value_type = String, example = "claude_code")]
    pub agent: AgentKind,
    pub title: String,
    pub context: Option<String>,
    pub tasks: Vec<String>,
    #[schema(value_type = Object)]
    pub status: Status,
    pub approvals: usize,
    pub started_at: Rfc3339,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionView {
    pub id: String,
    pub project: String,
    pub agent: AgentKind,
    pub title: String,
    pub context: Option<String>,
    pub tasks: Vec<String>,
    pub cwd: PathBuf,
    pub status: Status,
    pub started_at: Rfc3339,
    pub transcript: Transcript,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SessionEvent {
    Snapshot(Box<SessionView>),
    Agent(AgentEvent),
    Tasks { tasks: Vec<String> },
}

// The transcript and the written tasks move together because every send happens while this is
// held: a reader that takes its snapshot under the same lock can then subscribe without racing the
// events the snapshot already folded in.
#[derive(Debug, Default)]
struct Live {
    transcript: Transcript,
    tasks: Vec<String>,
}

pub struct AgentSession {
    id: String,
    project: String,
    // None while the project cannot be read; the tasks the agent writes then go unnoticed.
    abbreviation: Option<Abbreviation>,
    workspace: Workspace,
    kind: AgentKind,
    title: String,
    context: Option<String>,
    started_at: Timestamp,
    // None while no agent runs: after a restart, and after the agent exited.
    agent: AsyncMutex<Option<SessionHandle>>,
    live: Mutex<Live>,
    events: broadcast::Sender<SessionEvent>,
    publisher: Publisher,
    store: Arc<AgentStore>,
    done: AtomicBool,
    pump: Mutex<Option<JoinHandle<()>>>,
}

impl AgentSession {
    fn lock(&self) -> MutexGuard<'_, Live> {
        self.live.lock().expect("agent session lock poisoned")
    }

    // The list of sessions changes on a new status, an approval that opens or closes, and a task
    // written, not on each streamed token, so `/api/events` carries only these.
    fn apply(&self, event: AgentEvent) {
        let mut live = self.lock();
        let before = (live.transcript.status, live.transcript.approvals.len());
        live.transcript.apply(&event);
        let written = match &event {
            AgentEvent::ToolEnded(ToolOutcome {
                item,
                failed: false,
                ..
            }) => live
                .transcript
                .entries
                .iter()
                .find_map(|entry| match entry {
                    Entry::Tool {
                        item: at,
                        name,
                        input,
                        output,
                        ..
                    } if at == item => Some(written_tasks(name, input, output, self.abbreviation)),
                    _ => None,
                })
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        let mut wrote = false;
        for key in written {
            if !live.tasks.contains(&key) {
                live.tasks.push(key);
                wrote = true;
            }
        }
        let after = (live.transcript.status, live.transcript.approvals.len());
        let delta = streams(&event);
        let _ = self.events.send(SessionEvent::Agent(event));
        if wrote {
            let _ = self.events.send(SessionEvent::Tasks {
                tasks: live.tasks.clone(),
            });
        }
        if wrote || !delta {
            self.persist(&live);
        }
        if wrote || before != after {
            self.announce();
        }
    }

    fn persist(&self, live: &Live) {
        if !self.done.load(Ordering::Relaxed) {
            self.store.save(self.record(live));
        }
    }

    fn record(&self, live: &Live) -> StoredSession {
        StoredSession {
            id: self.id.clone(),
            project: self.project.clone(),
            agent: self.kind,
            title: self.title.clone(),
            context: self.context.clone(),
            tasks: live.tasks.clone(),
            started_at: self.started_at,
            transcript: live.transcript.clone(),
        }
    }

    // No agent event says that a session which exited starts again, so the stream readers take a
    // fresh snapshot instead.
    fn restart(&self) {
        let mut live = self.lock();
        live.transcript.status = Status::Starting;
        self.persist(&live);
        let _ = self
            .events
            .send(SessionEvent::Snapshot(Box::new(self.view(&live))));
        drop(live);
        self.announce();
    }

    fn announce(&self) {
        self.publisher.publish(ChangeEvent::AgentSessionsChanged {
            project: self.project.clone(),
        });
    }

    fn view(&self, live: &Live) -> SessionView {
        SessionView {
            id: self.id.clone(),
            project: self.project.clone(),
            agent: self.kind,
            title: self.title.clone(),
            context: self.context.clone(),
            tasks: live.tasks.clone(),
            cwd: self.workspace.cwd.clone(),
            status: live.transcript.status,
            started_at: self.started_at.into(),
            transcript: live.transcript.clone(),
        }
    }

    fn summary(&self) -> SessionSummary {
        let live = self.lock();
        SessionSummary {
            id: self.id.clone(),
            project: self.project.clone(),
            agent: self.kind,
            title: self.title.clone(),
            context: self.context.clone(),
            tasks: live.tasks.clone(),
            status: live.transcript.status,
            approvals: live.transcript.approvals.len(),
            started_at: self.started_at.into(),
        }
    }

    fn subscribe(&self) -> (SessionEvent, broadcast::Receiver<SessionEvent>) {
        let live = self.lock();
        let receiver = self.events.subscribe();
        (SessionEvent::Snapshot(Box::new(self.view(&live))), receiver)
    }
}

pub struct AgentSessions {
    agents: BTreeMap<AgentKind, Arc<dyn Agent>>,
    sessions: Mutex<BTreeMap<String, Arc<AgentSession>>>,
    store: Arc<AgentStore>,
}

impl AgentSessions {
    pub fn new(agents: BTreeMap<AgentKind, Arc<dyn Agent>>, store: AgentStore) -> Self {
        Self {
            agents,
            sessions: Mutex::new(BTreeMap::new()),
            store: Arc::new(store),
        }
    }

    pub(crate) fn backends(&self) -> BTreeMap<AgentKind, Arc<dyn Agent>> {
        self.agents.clone()
    }

    fn held(&self) -> MutexGuard<'_, BTreeMap<String, Arc<AgentSession>>> {
        self.sessions.lock().expect("agent sessions lock poisoned")
    }

    fn all(&self) -> Vec<Arc<AgentSession>> {
        let mut sessions: Vec<Arc<AgentSession>> = self.held().values().cloned().collect();
        sessions.sort_by_key(|session| std::cmp::Reverse(session.started_at));
        sessions
    }

    fn backend(&self, kind: AgentKind) -> Option<Arc<dyn Agent>> {
        self.agents.get(&kind).cloned()
    }

    fn get(&self, project: &str, id: &str) -> Option<Arc<AgentSession>> {
        self.held()
            .get(id)
            .filter(|session| session.project == project)
            .cloned()
    }

    // Every live session is asked to stop, then joined, so a daemon on its way out leaves no child
    // still writing the worktree it is about to release, and the store holds each session's last
    // event.
    pub async fn finish(&self) {
        let sessions: Vec<Arc<AgentSession>> = self.held().values().cloned().collect();
        for session in &sessions {
            let agent = session.agent.lock().await.clone();
            if let Some(agent) = agent {
                let _ = agent.shutdown().await;
            }
        }
        let pumps: Vec<JoinHandle<()>> = sessions
            .iter()
            .filter_map(|session| session.pump.lock().expect("pump lock poisoned").take())
            .collect();
        for pump in pumps {
            let _ = tokio::time::timeout(STOP_GRACE, pump).await;
        }
        let store = Arc::clone(&self.store);
        let _ = tokio::task::spawn_blocking(move || store.close()).await;
    }
}

pub fn backends() -> BTreeMap<AgentKind, Arc<dyn Agent>> {
    let claude = ClaudeCode::new()
        .tools(Tools::Only(
            TOOLS.iter().map(|name| (*name).to_owned()).collect(),
        ))
        .skills(Skills::Disabled);
    BTreeMap::from([
        (AgentKind::ClaudeCode, Arc::new(claude) as Arc<dyn Agent>),
        (AgentKind::Codex, Arc::new(Codex::new()) as Arc<dyn Agent>),
    ])
}

// A session the store kept comes back with no agent: the one it had went with the daemon that ran
// it. Its transcript settles as on an exit, and its next prompt resumes the agent's own session.
pub(crate) fn restore(state: &AppState, stored: Vec<StoredSession>) {
    for record in stored {
        let Some(project) = state.project(&record.project) else {
            tracing::info!(session = %record.id, project = %record.project, "kept an agent session of a project this daemon does not serve");
            continue;
        };
        let workspace = Workspace::of(&project);
        let mut transcript = record.transcript;
        if !matches!(transcript.status, Status::Exited { .. }) {
            transcript.apply(&AgentEvent::Exited { code: None });
        }
        let session = Arc::new(AgentSession {
            id: record.id.clone(),
            project: project.name(),
            abbreviation: project.index().abbreviation(),
            workspace,
            kind: record.agent,
            title: record.title,
            context: record.context,
            started_at: record.started_at,
            agent: AsyncMutex::new(None),
            live: Mutex::new(Live {
                transcript,
                tasks: record.tasks,
            }),
            events: broadcast::channel(EVENT_CHANNEL_CAPACITY).0,
            publisher: state.publisher().clone(),
            store: Arc::clone(&state.agents.store),
            done: AtomicBool::new(false),
            pump: Mutex::new(None),
        });
        state.agents.held().insert(record.id, session);
    }
}

// A delta is folded into the item it streams, and the item's end carries the whole text, so the
// store waits for that end.
fn streams(event: &AgentEvent) -> bool {
    matches!(
        event,
        AgentEvent::Thinking { .. }
            | AgentEvent::Message { .. }
            | AgentEvent::ToolOutput { .. }
            | AgentEvent::ResultDelta { .. }
    )
}

// A task deleted since the key was typed is no longer one the agent can open.
async fn open_task(project: &Arc<Project>, task: String) -> Result<String, ApiError> {
    let reading = Arc::clone(project);
    blocking(move || {
        let number = reading.number(&task)?;
        match reading.index().contains(number) {
            true => Ok(task),
            false => Err(no_such_task(&task)),
        }
    })
    .await
}

// The agent keeps its own session on disk, so a prompt after a restart can resume it.
async fn session_options(
    project: &Arc<Project>,
    workspace: &Workspace,
    kind: AgentKind,
    open: Option<&str>,
    resume: Option<SessionId>,
) -> Result<SessionOptions, ApiError> {
    let reading = Arc::clone(project);
    let tags = blocking(move || Ok(registered_tags(&reading))).await?;
    let options = SessionOptions::new(workspace.cwd.clone())
        .permissions(Permissions::Full)
        .mcp(McpPolicy::Disabled)
        .persistence(Persistence::Stored)
        .instructions(instructions(workspace, kind, open, &tags));
    let options = match resume {
        Some(session) => options.resume(session),
        None => options,
    };
    Ok(match kind {
        AgentKind::ClaudeCode => options.model(CLAUDE_MODEL),
        AgentKind::Codex => options,
    })
}

fn no_backend(kind: AgentKind) -> ApiError {
    ApiError::unavailable(format!("this daemon runs no {} agent", kind_name(kind)))
}

// The caller holds `session.agent`, and stores the handle this returns in it.
fn attach(state: &AppState, session: &Arc<AgentSession>, started: Session) -> SessionHandle {
    let (handle, events) = started.split();
    let pump = tokio::spawn(pump(Arc::clone(session), events, state.stopping()));
    *session.pump.lock().expect("pump lock poisoned") = Some(pump);
    handle
}

// The agent of a session, started first when none runs. A session whose agent exited, or whose
// daemon restarted, resumes the agent session its transcript names.
async fn running_agent(
    state: &AppState,
    session: &Arc<AgentSession>,
) -> Result<SessionHandle, ApiError> {
    let mut agent = session.agent.lock().await;
    // An agent that exited keeps its handle until its pump drains; that handle leads nowhere.
    let exited = matches!(session.lock().transcript.status, Status::Exited { .. });
    if let Some(handle) = agent.as_ref().filter(|_| !exited) {
        return Ok(handle.clone());
    }
    let project = project_of(state, &session.project)?;
    let backend = state
        .agents
        .backend(session.kind)
        .ok_or_else(|| no_backend(session.kind))?;
    // A task deleted since the session started is no longer one the agent can open.
    let open = match &session.context {
        Some(task) => open_task(&project, task.clone()).await.ok(),
        None => None,
    };
    let resume = session
        .lock()
        .transcript
        .info
        .as_ref()
        .map(|info| info.session.clone());
    let options = session_options(
        &project,
        &session.workspace,
        session.kind,
        open.as_deref(),
        resume,
    )
    .await?;
    let started = backend.start(options).map_err(spawn_error)?;
    session.restart();
    let handle = attach(state, session, started);
    *agent = Some(handle.clone());
    Ok(handle)
}

// The appended system prompt. The tasks live in the daemon, not in files, so every write goes
// through the CLI.
fn instructions(
    workspace: &Workspace,
    kind: AgentKind,
    open: Option<&str>,
    tags: &[String],
) -> String {
    let exe = std::env::current_exe()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| "openplan".to_owned());
    let cwd = workspace.cwd.display();
    let mut lines = vec![
        "You answer questions about the tasks of this project, and you write its tasks. You never \
         write code. A prompt that tells you about a defect or a feature asks you for a task about \
         it."
        .to_owned(),
        format!("The code is at {cwd}. Read it there. Never write there."),
        "The tasks are not files in the checkout. Read and write them only with the openplan CLI."
            .to_owned(),
        format!(
            "Create a task with `{exe} create \"<title>\" --tag <tag> --body-file <file>`. It \
             prints the key. Read a task with `{exe} get <key>`."
        ),
        match tags.is_empty() {
            true => "This project registers no tag. Create the task with no `--tag`.".to_owned(),
            false => format!(
                "These are the registered tags: {}. Use only these names.",
                tags.join(", ")
            ),
        },
        format!("Find the tasks that are already written with `{exe} search <query>`."),
        format!(
            "Edit an existing task this way: write the output of `{exe} get <key>` to a file, edit \
             the file, then run `{exe} write <key> --file <file>`. Keep every comment in the file. \
             Run `{exe} lint` after each edit."
        ),
        "Never run git, and never create a worktree.".to_owned(),
        "Prefer a diagram to prose. When a task body describes how parts connect, how data \
         flows, or what order events take, draw it as a fenced code block tagged `d2`. The UI \
         renders it. Use plain d2: shapes, containers, connections, sequence diagrams, and \
         tables. Do not use imports, icons, links, or layout settings. Write prose only for a \
         rule, a reason, or a number."
            .to_owned(),
        "When you write a task, give its key in your answer. Do not report what you may not do."
            .to_owned(),
    ];
    // The commands carry the key, because an agent told only the key spent its first turn on a
    // search for the task.
    if let Some(key) = open {
        lines.push(format!(
            "The user has task {key} open. A prompt that names no task is about this task. Read \
             it with `{exe} get {key}`. Replace it with `{exe} write {key} --file <file>`. Change \
             one field with `{exe} set {key} <field> <value>`. The task is not a file, so do not \
             search for it. Do not confirm which task it is."
        ));
    }
    // Claude Code reads the repository's CLAUDE.md; Codex does not, so its rules are repeated.
    if kind == AgentKind::Codex {
        lines.push(
            "Write all prose in ASD-STE100 Simplified Technical English. Use the active voice. \
             Write short sentences. Give one instruction in each sentence."
                .to_owned(),
        );
        lines.push(
            "Give each task one feature slice. Never write a parent task with a hand-written list \
             of children."
                .to_owned(),
        );
    }
    lines.join("\n")
}

// Naming the tags in the prompt spares the agent a `tag list` turn, and a turn reads the whole
// context again. A plan that cannot be read names none, which the prompt answers for.
fn registered_tags(project: &Project) -> Vec<String> {
    project
        .tracker()
        .plan()
        .map(|plan| plan.tag_names().iter().cloned().collect())
        .unwrap_or_default()
}

// What a finished tool call wrote, read from the call itself. The CLI names only the kind of agent
// that ran it, and two sessions of one kind can run at once.
fn written_tasks(
    tool: &str,
    input: &Value,
    output: &str,
    abbreviation: Option<Abbreviation>,
) -> Vec<String> {
    let Some(abbreviation) = abbreviation else {
        return Vec::new();
    };
    if tool != "Bash" && tool != "shell" {
        return Vec::new();
    }
    let command = match input.get("command") {
        Some(Value::String(command)) => command.clone(),
        Some(Value::Array(words)) => words
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(" "),
        _ => return Vec::new(),
    };
    cli_writes(&command, output, &abbreviation)
}

// `create` prints the key it made and nothing else; every other write names its task first.
fn cli_writes(command: &str, output: &str, abbreviation: &Abbreviation) -> Vec<String> {
    let words: Vec<&str> = command
        .split_whitespace()
        .map(|word| word.trim_matches(|c: char| c == '"' || c == '\'' || c == ';'))
        .collect();
    let Some(exe) = words.iter().position(|word| word.ends_with("openplan")) else {
        return Vec::new();
    };
    let Some(verb) = words[exe + 1..]
        .iter()
        .position(|word| TASK_WRITES.contains(word))
        .map(|at| exe + 1 + at)
    else {
        return Vec::new();
    };
    if words[verb] == "create" {
        return output
            .lines()
            .map(str::trim)
            .filter(|line| abbreviation.parse_key(line).is_some())
            .map(str::to_owned)
            .collect();
    }
    words[verb + 1..]
        .iter()
        .find(|word| abbreviation.parse_key(word).is_some())
        .map(|key| vec![(*key).to_owned()])
        .unwrap_or_default()
}

fn session_id() -> String {
    let mut bytes = [0u8; SESSION_ID_BYTES];
    // A failing system RNG leaves no id anyone could guess, and the daemon holds nothing worth
    // guessing across a restart, so the clock is a good enough fallback.
    if getrandom::fill(&mut bytes).is_err() {
        bytes = Timestamp::now().as_nanosecond().to_le_bytes()[..SESSION_ID_BYTES]
            .try_into()
            .expect("the slice is exactly as long as the array");
    }
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn no_such_session(id: &str) -> ApiError {
    ApiError::new(
        StatusCode::NOT_FOUND,
        format!("no such agent session: {id}; it was marked done, or it never existed"),
    )
}

fn workspace_of(state: &AppState, project: &str) -> Result<(Arc<Project>, Workspace), ApiError> {
    let project = project_of(state, project)?;
    let workspace = Workspace::of(&project);
    Ok((project, workspace))
}

fn session_of(state: &AppState, project: &str, id: &str) -> Result<Arc<AgentSession>, ApiError> {
    let (project, _) = workspace_of(state, project)?;
    state
        .agents
        .get(&project.name(), id)
        .ok_or_else(|| no_such_session(id))
}

#[utoipa::path(
    get,
    path = "/api/agent/sessions",
    responses(
        (status = 200, description = "Every session this daemon holds, in every project, newest first", body = Vec<SessionSummary>)
    )
)]
pub(crate) async fn list_sessions(State(state): State<AppState>) -> Json<Vec<SessionSummary>> {
    Json(
        state
            .agents
            .all()
            .iter()
            .map(|session| session.summary())
            .collect(),
    )
}

#[utoipa::path(
    post,
    path = "/api/projects/{project}/agent/sessions",
    params(("project" = String, Path, description = "Project name")),
    request_body = CreateSession,
    responses(
        (status = 201, description = "The session started and took the prompt", body = CreatedSession),
        (status = 400, description = "The context task key is invalid", body = ApiErrorBody),
        (status = 404, description = "No such project, or no such context task", body = ApiErrorBody),
        (status = 503, description = "The project is not being served, or the agent could not start", body = ApiErrorBody)
    )
)]
pub(crate) async fn create_session(
    State(state): State<AppState>,
    Path(project): Path<String>,
    Json(body): Json<CreateSession>,
) -> Result<Response, ApiError> {
    let (project, workspace) = workspace_of(&state, &project)?;
    let kind = body.agent.unwrap_or(AgentKind::ClaudeCode);
    let backend = state.agents.backend(kind).ok_or_else(|| no_backend(kind))?;
    let open = match body.context.clone() {
        Some(task) => Some(open_task(&project, task).await?),
        None => None,
    };
    let options = session_options(&project, &workspace, kind, open.as_deref(), None).await?;
    let started = backend.start(options).map_err(spawn_error)?;

    let id = session_id();
    let session = Arc::new(AgentSession {
        id: id.clone(),
        project: project.name(),
        abbreviation: project.index().abbreviation(),
        workspace,
        kind,
        title: body.prompt.clone(),
        context: body.context,
        started_at: Timestamp::now(),
        agent: AsyncMutex::new(None),
        live: Mutex::new(Live::default()),
        events: broadcast::channel(EVENT_CHANNEL_CAPACITY).0,
        publisher: state.publisher().clone(),
        store: Arc::clone(&state.agents.store),
        done: AtomicBool::new(false),
        pump: Mutex::new(None),
    });
    let handle = {
        let mut agent = session.agent.lock().await;
        let handle = attach(&state, &session, started);
        *agent = Some(handle.clone());
        handle
    };
    session.persist(&session.lock());
    state.agents.held().insert(id.clone(), Arc::clone(&session));
    session.announce();

    handle.prompt(body.prompt).await.map_err(spawn_error)?;
    Ok((StatusCode::CREATED, Json(CreatedSession { id })).into_response())
}

#[utoipa::path(
    post,
    path = "/api/projects/{project}/agent/sessions/{id}/prompt",
    params(
        ("project" = String, Path, description = "Project name"),
        ("id" = String, Path, description = "Session id")
    ),
    request_body = Say,
    responses(
        (status = 202, description = "The agent took the prompt"),
        (status = 404, description = "No such project, or no such session", body = ApiErrorBody),
        (status = 409, description = "A turn is already running", body = ApiErrorBody),
        (status = 503, description = "The project is not being served, or the agent could not start", body = ApiErrorBody)
    )
)]
pub(crate) async fn prompt_session(
    State(state): State<AppState>,
    Path((project, id)): Path<(String, String)>,
    Json(body): Json<Say>,
) -> Result<StatusCode, ApiError> {
    let session = session_of(&state, &project, &id)?;
    if session.lock().transcript.running() {
        return Err(ApiError::conflict(
            "this session is running a turn; interrupt it or wait for it to end",
        ));
    }
    running_agent(&state, &session)
        .await?
        .prompt(body.text)
        .await
        .map_err(spawn_error)?;
    Ok(StatusCode::ACCEPTED)
}

#[utoipa::path(
    post,
    path = "/api/projects/{project}/agent/sessions/{id}/interrupt",
    params(
        ("project" = String, Path, description = "Project name"),
        ("id" = String, Path, description = "Session id")
    ),
    responses(
        (status = 202, description = "The agent took the interrupt"),
        (status = 404, description = "No such project, or no such session", body = ApiErrorBody),
        (status = 409, description = "No agent runs for the session", body = ApiErrorBody),
        (status = 503, description = "The project is not being served, or the agent has stopped", body = ApiErrorBody)
    )
)]
pub(crate) async fn interrupt_session(
    State(state): State<AppState>,
    Path((project, id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let session = session_of(&state, &project, &id)?;
    let Some(agent) = session.agent.lock().await.clone() else {
        return Err(ApiError::conflict(
            "no agent runs for this session, so it has no turn to interrupt",
        ));
    };
    agent.interrupt().await.map_err(spawn_error)?;
    Ok(StatusCode::ACCEPTED)
}

#[utoipa::path(
    post,
    path = "/api/projects/{project}/agent/sessions/{id}/approvals/{approval}",
    params(
        ("project" = String, Path, description = "Project name"),
        ("id" = String, Path, description = "Session id"),
        ("approval" = String, Path, description = "Approval id, as the transcript reports it")
    ),
    request_body = Decision,
    responses(
        (status = 202, description = "The agent took the decision"),
        (status = 404, description = "No such project, no such session, or no such open approval", body = ApiErrorBody),
        (status = 503, description = "The project is not being served, or the agent has stopped", body = ApiErrorBody)
    )
)]
pub(crate) async fn approve(
    State(state): State<AppState>,
    Path((project, id, approval)): Path<(String, String, String)>,
    Json(body): Json<Decision>,
) -> Result<StatusCode, ApiError> {
    let session = session_of(&state, &project, &id)?;
    let approval = ApprovalId(approval);
    let open = session
        .lock()
        .transcript
        .approvals
        .iter()
        .any(|request| request.id == approval);
    if !open {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            format!(
                "this session holds no open approval {}; it was answered, or the turn ended",
                approval.0
            ),
        ));
    }
    let agent = session
        .agent
        .lock()
        .await
        .clone()
        .ok_or_else(|| ApiError::unavailable("the agent of this session has stopped"))?;
    agent.approve(approval, body.0).await.map_err(spawn_error)?;
    Ok(StatusCode::ACCEPTED)
}

#[utoipa::path(
    delete,
    path = "/api/projects/{project}/agent/sessions/{id}",
    params(
        ("project" = String, Path, description = "Project name"),
        ("id" = String, Path, description = "Session id")
    ),
    responses(
        (status = 204, description = "The session is marked done: its agent stops, and neither the list nor a restart brings it back"),
        (status = 404, description = "No such project, or no such session", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
pub(crate) async fn delete_session(
    State(state): State<AppState>,
    Path((project, id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let session = session_of(&state, &project, &id)?;
    session.done.store(true, Ordering::Relaxed);
    state.agents.held().remove(&session.id);
    state.agents.store.done(&session.id, Timestamp::now());
    session.announce();
    let agent = session.agent.lock().await.clone();
    if let Some(agent) = agent {
        let _ = agent.shutdown().await;
    }
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn session_events(
    State(state): State<AppState>,
    Path((project, id)): Path<(String, String)>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let session = session_of(&state, &project, &id)?;
    let (snapshot, mut events) = session.subscribe();
    let mut stopping = state.stopping();
    let (tx, rx) = mpsc::channel(EVENT_CHANNEL_CAPACITY);

    tokio::spawn(async move {
        if tx.send(Ok(encode(&snapshot))).await.is_err() {
            return;
        }
        loop {
            let event = tokio::select! {
                _ = tx.closed() => return,
                _ = stopping.wait_for(|&stopping| stopping) => return,
                message = events.recv() => match message {
                    Ok(event) => event,
                    // The reader missed events, so its transcript no longer matches this session.
                    // Ending the stream is what makes it reconnect and take a fresh snapshot.
                    Err(broadcast::error::RecvError::Lagged(_)) => return,
                    Err(broadcast::error::RecvError::Closed) => return,
                },
            };
            tokio::select! {
                result = tx.send(Ok(encode(&event))) => {
                    if result.is_err() {
                        return;
                    }
                }
                _ = stopping.wait_for(|&stopping| stopping) => return,
            }
        }
    });

    Ok(Sse::new(ReceiverStream::new(rx)).keep_alive(KeepAlive::default()))
}

fn encode(event: &SessionEvent) -> Event {
    Event::default()
        .json_data(event)
        .unwrap_or_else(|_| Event::default().comment("failed to encode session event"))
}

fn kind_name(kind: AgentKind) -> &'static str {
    match kind {
        AgentKind::ClaudeCode => "claude_code",
        AgentKind::Codex => "codex",
    }
}

fn spawn_error(err: AgentError) -> ApiError {
    ApiError::unavailable(err.to_string())
}

async fn pump(
    session: Arc<AgentSession>,
    mut events: mpsc::Receiver<AgentEvent>,
    mut stopping: watch::Receiver<bool>,
) {
    let mut asked = false;
    loop {
        tokio::select! {
            event = events.recv() => match event {
                Some(event) => session.apply(event),
                None => break,
            },
            _ = async { let _ = stopping.wait_for(|&stopping| stopping).await; }, if !asked => {
                asked = true;
                let agent = session.agent.lock().await.clone();
                if let Some(agent) = agent {
                    let _ = agent.shutdown().await;
                }
            }
        }
    }
    // The session stays: the reader can still read the transcript and the reason, and a prompt
    // starts its agent again. A prompt may have started that new agent already; this pump is then no
    // longer the session's, and leaves the new handle alone.
    let mut agent = session.agent.lock().await;
    let current = session
        .pump
        .lock()
        .expect("pump lock poisoned")
        .as_ref()
        .is_some_and(|pump| pump.id() == tokio::task::id());
    if current {
        *agent = None;
    }
}
