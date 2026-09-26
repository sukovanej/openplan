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
    Agent, AgentError, AgentEvent, AgentKind, ApprovalDecision, ApprovalId, McpPolicy, Permissions,
    Persistence, SessionHandle, SessionOptions, Status, Transcript,
};
use op_agent_claude::{ClaudeCode, Skills, Tools};
use op_agent_codex::Codex;
use op_api::{ApiErrorBody, ChangeEvent, Rfc3339};
use op_task::Timestamp;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, mpsc, watch};
use tokio::task::JoinHandle;
use tokio_stream::Stream;
use tokio_stream::wrappers::ReceiverStream;
use utoipa::ToSchema;

use crate::tasks::no_such_task;
use crate::{ApiError, AppState, EVENT_CHANNEL_CAPACITY, Project, Published, blocking, project_of};

const SESSION_ID_BYTES: usize = 8;
// The driver kills a child that ignores a closed stdin after five seconds, so a stop that waits
// past that mark sees every session out rather than cutting one short.
const STOP_GRACE: Duration = Duration::from_secs(6);
const TOOLS: [&str; 6] = ["Read", "Grep", "Glob", "Bash", "Edit", "Write"];

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
    pub task: Option<String>,
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

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SessionSummary {
    pub id: String,
    #[schema(value_type = String, example = "claude_code")]
    pub agent: AgentKind,
    pub task: Option<String>,
    #[schema(value_type = Object)]
    pub status: Status,
    pub started_at: Rfc3339,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionView {
    pub id: String,
    pub project: String,
    pub agent: AgentKind,
    pub task: Option<String>,
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
    Task { id: String },
}

// The transcript and the bound task move together because every send happens while this is held:
// a reader that takes its snapshot under the same lock can then subscribe without racing the
// events the snapshot already folded in.
#[derive(Debug, Default)]
struct Live {
    transcript: Transcript,
    task: Option<String>,
}

pub struct AgentSession {
    id: String,
    project: String,
    workspace: Workspace,
    kind: AgentKind,
    started_at: Timestamp,
    handle: SessionHandle,
    live: Mutex<Live>,
    events: broadcast::Sender<SessionEvent>,
    closing: AtomicBool,
    pump: Mutex<Option<JoinHandle<()>>>,
}

impl AgentSession {
    fn lock(&self) -> MutexGuard<'_, Live> {
        self.live.lock().expect("agent session lock poisoned")
    }

    fn apply(&self, event: AgentEvent) {
        let mut live = self.lock();
        live.transcript.apply(&event);
        let _ = self.events.send(SessionEvent::Agent(event));
    }

    // The first task this session's agent writes while a turn runs is the task it works on. The
    // CLI names the agent that ran it, so a person's edit in the same seconds does not bind.
    fn bind(&self, change: &Published) {
        let ChangeEvent::TaskChanged { project, id } = &change.event else {
            return;
        };
        if project != &self.project || change.via.as_deref() != Some(agent_token(self.kind)) {
            return;
        }
        let mut live = self.lock();
        if live.task.is_some() || !live.transcript.running() {
            return;
        }
        live.task = Some(id.clone());
        let _ = self.events.send(SessionEvent::Task { id: id.clone() });
    }

    fn view(&self, live: &Live) -> SessionView {
        SessionView {
            id: self.id.clone(),
            project: self.project.clone(),
            agent: self.kind,
            task: live.task.clone(),
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
            agent: self.kind,
            task: live.task.clone(),
            status: live.transcript.status,
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
}

impl AgentSessions {
    pub fn new(agents: BTreeMap<AgentKind, Arc<dyn Agent>>) -> Self {
        Self {
            agents,
            sessions: Mutex::new(BTreeMap::new()),
        }
    }

    fn held(&self) -> MutexGuard<'_, BTreeMap<String, Arc<AgentSession>>> {
        self.sessions.lock().expect("agent sessions lock poisoned")
    }

    fn of_project(&self, project: &str) -> Vec<Arc<AgentSession>> {
        let mut sessions: Vec<Arc<AgentSession>> = self
            .held()
            .values()
            .filter(|session| session.project == project)
            .cloned()
            .collect();
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
    // still writing the worktree it is about to release.
    pub async fn finish(&self) {
        let sessions: Vec<Arc<AgentSession>> = self.held().values().cloned().collect();
        for session in &sessions {
            session.closing.store(true, Ordering::Relaxed);
            let _ = session.handle.shutdown().await;
        }
        let pumps: Vec<JoinHandle<()>> = sessions
            .iter()
            .filter_map(|session| session.pump.lock().expect("pump lock poisoned").take())
            .collect();
        for pump in pumps {
            let _ = tokio::time::timeout(STOP_GRACE, pump).await;
        }
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

// The tokens the CLI sends for the agents this daemon runs, as `author::agent` spells them.
fn agent_token(kind: AgentKind) -> &'static str {
    match kind {
        AgentKind::ClaudeCode => "claude-code",
        AgentKind::Codex => "codex",
    }
}

// The appended system prompt. The tasks live in the daemon, not in files, so every write goes
// through the CLI.
fn instructions(workspace: &Workspace, kind: AgentKind, task: Option<&str>) -> String {
    let exe = std::env::current_exe()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| "openplan".to_owned());
    let cwd = workspace.cwd.display();
    let mut lines = vec![
        format!("The code is at {cwd}. Read it there. Never write there."),
        "The tasks are not files in the checkout. Read and write them only with the openplan CLI."
            .to_owned(),
        format!(
            "Create a task with `{exe} create \"<title>\" --tag <tag> --body-file <file>`. It \
             prints the key. Read a task with `{exe} get <key>`. List the registered tags with \
             `{exe} tag list` and use only those names."
        ),
        format!(
            "Edit an existing task this way: write the output of `{exe} get <key>` to a file, edit \
             the file, then run `{exe} write <key> --file <file>`. Keep every comment in the file. \
             Run `{exe} lint` after each edit."
        ),
        "Never run git, and never create a worktree.".to_owned(),
        format!(
            "Prefer a diagram to prose. When a task body describes how parts connect, how data \
             flows, or what order events take, draw it as a fenced code block tagged `mermaid`. \
             The daemon draws it, and `{exe} lint` reports a block that does not parse. Use only \
             `flowchart`, `sequenceDiagram`, and `erDiagram`. Do not use `%%{{init}}%%`, front \
             matter, `@{{ }}` shapes, `click`, `classDef`, or `style`. Write prose only for a \
             rule, a reason, or a number."
        ),
    ];
    if let Some(task) = task {
        lines.push(format!(
            "This session works on task {task}. Write that task and no other."
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
        format!("no such agent session: {id}; the daemon keeps none across a restart"),
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
    path = "/api/projects/{project}/agent/sessions",
    params(("project" = String, Path, description = "Project name")),
    responses(
        (status = 200, description = "The sessions this daemon holds for the project, newest first", body = Vec<SessionSummary>),
        (status = 404, description = "No such project", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
pub(crate) async fn list_sessions(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> Result<Json<Vec<SessionSummary>>, ApiError> {
    let (project, _) = workspace_of(&state, &project)?;
    let summaries = state
        .agents
        .of_project(&project.name())
        .iter()
        .map(|session| session.summary())
        .collect();
    Ok(Json(summaries))
}

#[utoipa::path(
    post,
    path = "/api/projects/{project}/agent/sessions",
    params(("project" = String, Path, description = "Project name")),
    request_body = CreateSession,
    responses(
        (status = 201, description = "The session started and took the prompt", body = CreatedSession),
        (status = 400, description = "The task key is invalid", body = ApiErrorBody),
        (status = 404, description = "No such project, or no such task", body = ApiErrorBody),
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
    let agent = state.agents.backend(kind).ok_or_else(|| {
        ApiError::unavailable(format!("this daemon runs no {} agent", kind_name(kind)))
    })?;

    if let Some(task) = body.task.clone() {
        let reading = Arc::clone(&project);
        blocking(move || {
            let number = reading.number(&task)?;
            match reading.index().contains(number) {
                true => Ok(()),
                false => Err(no_such_task(&task)),
            }
        })
        .await?;
    }

    let options = SessionOptions::new(workspace.cwd.clone())
        .permissions(Permissions::Full)
        .mcp(McpPolicy::Disabled)
        .persistence(Persistence::Ephemeral)
        .instructions(instructions(&workspace, kind, body.task.as_deref()));
    let started = agent.start(options).map_err(spawn_error)?;

    let id = session_id();
    let (handle, events) = started.split();
    let session = Arc::new(AgentSession {
        id: id.clone(),
        project: project.name(),
        workspace,
        kind,
        started_at: Timestamp::now(),
        handle: handle.clone(),
        live: Mutex::new(Live {
            transcript: Transcript::default(),
            task: body.task,
        }),
        events: broadcast::channel(EVENT_CHANNEL_CAPACITY).0,
        closing: AtomicBool::new(false),
        pump: Mutex::new(None),
    });
    // Registered before the pump runs, so a pump that ends at once — a daemon already stopping —
    // cannot try to remove an entry that is not there yet and leave it behind.
    state.agents.held().insert(id.clone(), Arc::clone(&session));
    let pump = tokio::spawn(pump(
        Arc::clone(&state.agents),
        Arc::clone(&session),
        events,
        state.publisher().subscribe(),
        state.stopping(),
    ));
    *session.pump.lock().expect("pump lock poisoned") = Some(pump);

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
        (status = 503, description = "The project is not being served, or the agent has stopped", body = ApiErrorBody)
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
    session
        .handle
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
        (status = 503, description = "The project is not being served, or the agent has stopped", body = ApiErrorBody)
    )
)]
pub(crate) async fn interrupt_session(
    State(state): State<AppState>,
    Path((project, id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let session = session_of(&state, &project, &id)?;
    session.handle.interrupt().await.map_err(spawn_error)?;
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
    session
        .handle
        .approve(approval, body.0)
        .await
        .map_err(spawn_error)?;
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
        (status = 204, description = "The agent was asked to stop; the entry goes when it exits"),
        (status = 404, description = "No such project, or no such session", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
pub(crate) async fn delete_session(
    State(state): State<AppState>,
    Path((project, id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let session = session_of(&state, &project, &id)?;
    session.closing.store(true, Ordering::Relaxed);
    let _ = session.handle.shutdown().await;
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
    sessions: Arc<AgentSessions>,
    session: Arc<AgentSession>,
    mut events: mpsc::Receiver<AgentEvent>,
    mut changes: broadcast::Receiver<Published>,
    mut stopping: watch::Receiver<bool>,
) {
    let mut asked = false;
    loop {
        tokio::select! {
            event = events.recv() => match event {
                Some(event) => session.apply(event),
                None => break,
            },
            change = changes.recv() => {
                if let Ok(change) = change {
                    session.bind(&change);
                }
            }
            _ = async { let _ = stopping.wait_for(|&stopping| stopping).await; }, if !asked => {
                asked = true;
                session.closing.store(true, Ordering::Relaxed);
                let _ = session.handle.shutdown().await;
            }
        }
    }
    // An agent that exited on its own keeps its entry, so the page can still read the transcript
    // and the reason. Only a delete or a stop takes it away.
    if session.closing.load(Ordering::Relaxed) {
        sessions.held().remove(&session.id);
    }
}
