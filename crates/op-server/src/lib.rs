use std::collections::{BTreeMap, VecDeque};
use std::convert::Infallible;
use std::future::Future;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use axum::{
    Json, Router,
    extract::{MatchedPath, Path, Query, State},
    http::{HeaderMap, HeaderValue, Request, StatusCode, Uri, header},
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
    routing::get,
};
use op_api::{
    ApiErrorBody, ChangeEvent, DaemonInfo, FlowCycles, KeyError, ProjectView, Refusal,
    RegisterProject, RenameProject, SourcePosition, StopReason,
};
use op_backend::{Actor, BackendError};
use op_tracker::TrackerError;
use rust_embed::RustEmbed;
use tokio::sync::{broadcast, mpsc, watch};
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tokio_stream::wrappers::{BroadcastStream, ReceiverStream};
use tokio_stream::{Stream, StreamExt as _};
use tower_http::classify::ServerErrorsFailureClass;
use tower_http::trace::TraceLayer;
use tracing::Span;
use utoipa::OpenApi;
use utoipa::openapi::RefOr;
use utoipa::openapi::schema::{AdditionalProperties, ArrayItems, Schema};
use utoipa_axum::{router::OpenApiRouter, routes};
use utoipa_swagger_ui::SwaggerUi;

pub mod agent;
mod drawing;
mod project;
mod registry;
mod tasks;
use agent::AgentSessions;
pub use drawing::DrawingCache;
pub use project::{
    Location, OpenError, Project, STORE_DIR, machine_actor, open_backend, sync_view,
};
pub use registry::{
    ProjectEntry, ProjectRegistry, REGISTRY_FILE, RegistryError, canonical, same_path, unique_name,
};

pub(crate) const EVENT_CHANNEL_CAPACITY: usize = 256;
// How many published events a reconnecting client can catch up on before it must read everything
// again.
const REPLAY: usize = 1024;
const SLOW_REQUEST: Duration = Duration::from_millis(1000);
const LAST_EVENT_ID: &str = "last-event-id";

#[derive(RustEmbed)]
#[folder = "../../web/packages/app/dist"]
struct Assets;

#[derive(Debug, thiserror::Error)]
pub enum ProjectsError {
    #[error(transparent)]
    Open(#[from] OpenError),
    #[error(transparent)]
    Registry(#[from] RegistryError),
    #[error("this daemon serves a fixed set of projects; it has no registry to write")]
    NoRegistry,
    #[error("no such project: {0}")]
    NoSuchProject(String),
    #[error("the project name {0:?} is already taken")]
    NameTaken(String),
    #[error("not a usable project name: {0:?}; use lowercase letters, digits, and dashes")]
    BadName(String),
    // The daemon runs in its own home directory, so a relative path would resolve there rather than
    // where the caller stands.
    #[error("a project path must be absolute, and {0} is not")]
    RelativePath(PathBuf),
    #[error("not an abbreviation: {0:?}; use exactly three uppercase letters")]
    BadAbbreviation(String),
    #[error(transparent)]
    Tracker(#[from] TrackerError),
}

// One published change, numbered so a client that reconnects can ask for what it missed. `via` is
// the agent that made the change, which only the daemon itself reads.
#[derive(Debug, Clone)]
pub(crate) struct Published {
    pub seq: u64,
    pub event: ChangeEvent,
    pub via: Option<String>,
}

#[derive(Clone)]
pub(crate) struct Publisher {
    tx: broadcast::Sender<Published>,
    log: Arc<EventLog>,
}

// `boot` tells two daemon lifetimes apart, so a cursor from before a restart is never read as one
// from this one.
struct EventLog {
    boot: String,
    recent: Mutex<(u64, VecDeque<Published>)>,
}

impl Publisher {
    fn new() -> Self {
        Self {
            tx: broadcast::channel(EVENT_CHANNEL_CAPACITY).0,
            log: Arc::new(EventLog {
                boot: format!("{:x}", op_backend::now().as_second()),
                recent: Mutex::new((0, VecDeque::new())),
            }),
        }
    }

    pub fn publish(&self, event: ChangeEvent, via: Option<String>) {
        // Every sync reports its status, twice a minute for each project, even when nothing moved.
        match event {
            ChangeEvent::SyncChanged { .. } => tracing::debug!(?event, "change published"),
            _ => tracing::info!(?event, "change published"),
        }
        let mut recent = self.log.recent.lock().expect("event log poisoned");
        recent.0 += 1;
        let published = Published {
            seq: recent.0,
            event,
            via,
        };
        recent.1.push_back(published.clone());
        if recent.1.len() > REPLAY {
            recent.1.pop_front();
        }
        // Sent under the lock, so the order on the channel is the order of the numbers.
        let _ = self.tx.send(published);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Published> {
        self.tx.subscribe()
    }

    fn id(&self, seq: u64) -> String {
        format!("{}-{seq}", self.log.boot)
    }

    // What came after `cursor`, or `None` when this daemon cannot say: another lifetime, or a gap
    // older than the replay window.
    fn since(&self, cursor: &str) -> Option<Vec<Published>> {
        let (boot, seq) = cursor.rsplit_once('-')?;
        let seq: u64 = seq.parse().ok()?;
        if boot != self.log.boot {
            return None;
        }
        let recent = self.log.recent.lock().expect("event log poisoned");
        let oldest = recent.1.front().map_or(recent.0 + 1, |first| first.seq);
        if seq + 1 < oldest {
            return None;
        }
        Some(
            recent
                .1
                .iter()
                .filter(|published| published.seq > seq)
                .cloned()
                .collect(),
        )
    }
}

#[derive(Clone)]
pub struct AppState {
    projects: Arc<RwLock<BTreeMap<String, Arc<Project>>>>,
    // Absent for a state built from a fixed project list, which has no file to keep in step.
    registry: Option<Arc<PathBuf>>,
    shutdown: Arc<watch::Sender<Option<StopReason>>>,
    health: Option<Arc<DaemonInfo>>,
    publisher: Publisher,
    agents: Arc<AgentSessions>,
    drawings: Arc<DrawingCache>,
}

impl AppState {
    // A repeated name keeps the first project of that name.
    pub fn new(projects: impl IntoIterator<Item = Project>) -> Self {
        let mut map: BTreeMap<String, Arc<Project>> = BTreeMap::new();
        for project in projects {
            let name = project.name();
            if map.contains_key(&name) {
                continue;
            }
            map.insert(name, Arc::new(project));
        }
        Self {
            projects: Arc::new(RwLock::new(map)),
            registry: None,
            shutdown: Arc::new(watch::channel(None).0),
            health: None,
            publisher: Publisher::new(),
            agents: Arc::new(AgentSessions::new(agent::backends())),
            drawings: Arc::new(DrawingCache::new(DrawingCache::BUDGET)),
        }
    }

    // A test hands in a fake here; the daemon keeps the two real backends `new` installed.
    pub fn with_agents(
        mut self,
        agents: BTreeMap<op_agent::AgentKind, Arc<dyn op_agent::Agent>>,
    ) -> Self {
        self.agents = Arc::new(AgentSessions::new(agents));
        self
    }

    pub fn agents(&self) -> Arc<AgentSessions> {
        Arc::clone(&self.agents)
    }

    pub fn drawings(&self) -> Arc<DrawingCache> {
        Arc::clone(&self.drawings)
    }

    pub fn with_registry(mut self, path: PathBuf) -> Self {
        self.registry = Some(Arc::new(path));
        self
    }

    pub fn with_health(mut self, info: DaemonInfo) -> Self {
        self.health = Some(Arc::new(info));
        self
    }

    pub fn project(&self, name: &str) -> Option<Arc<Project>> {
        self.read_projects().get(name).cloned()
    }

    pub fn projects(&self) -> Vec<Arc<Project>> {
        self.read_projects().values().cloned().collect()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ChangeEvent> {
        let (tx, rx) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        let mut published = self.publisher.subscribe();
        tokio::spawn(async move {
            while let Ok(next) = published.recv().await {
                if tx.send(next.event).is_err() {
                    return;
                }
            }
        });
        rx
    }

    // Each project starts listening to its backend and syncing with its remote.
    pub fn start_projects(&self) {
        for project in self.projects() {
            project.start(self.publisher.clone());
        }
    }

    // Idempotent by where the tasks live, not by the path asked for: two worktrees of one
    // repository are one project, and two concurrent first writes from the CLI both land. The bool
    // says whether this call added the project. An abbreviation starts the project's tasks.
    pub fn register(
        &self,
        request: &RegisterProject,
    ) -> Result<(ProjectView, bool), ProjectsError> {
        self.register_as(request, None)
    }

    // `author` signs the revision that starts the tasks; `None` signs it with the project's own
    // identity.
    pub fn register_as(
        &self,
        request: &RegisterProject,
        author: Option<Actor>,
    ) -> Result<(ProjectView, bool), ProjectsError> {
        let path = PathBuf::from(&request.path);
        if path.is_relative() {
            return Err(ProjectsError::RelativePath(path));
        }
        let abbreviation = request
            .abbreviation
            .as_deref()
            .map(|text| {
                text.parse::<op_task::Abbreviation>()
                    .map_err(|_| ProjectsError::BadAbbreviation(text.to_owned()))
            })
            .transpose()?;
        let registry_path = self.registry_path()?;
        // Starting tasks where some already live joins them; only a path with none takes the kind
        // its place suggests.
        let location = match (request.backend, abbreviation) {
            (Some(kind), _) => Location::find(&path, Some(kind))?,
            (None, None) => Location::find_or_join(&path)?,
            (None, Some(_)) => match Location::find(&path, None) {
                Err(OpenError::NoStore(_)) => Location::find(
                    &path,
                    Some(match op_backend_git::inspect(&path) {
                        Some(_) => op_api::BackendKind::Git,
                        None => op_api::BackendKind::Local,
                    }),
                )?,
                found => found?,
            },
        };
        let (project, created) = match self.serving(&location) {
            Some(existing) => (existing, false),
            // Opening reads every task, so it runs before the lock that every request takes.
            None => {
                let registry = ProjectRegistry::read(&registry_path)?.unwrap_or_default();
                let name = project_name(&registry, &self.read_projects(), &location)?;
                let opened = Arc::new(Project::open(name, location.clone())?);
                let mut projects = self.write_projects();
                match projects
                    .values()
                    .find(|project| project.location().key() == location.key())
                {
                    Some(existing) => (Arc::clone(existing), false),
                    None => {
                        let mut registry =
                            ProjectRegistry::read(&registry_path)?.unwrap_or_default();
                        let name = project_name(&registry, &projects, &location)?;
                        opened.rename(&name);
                        if !registry.holds_name(&name) {
                            registry.insert(ProjectEntry {
                                name: name.clone(),
                                path: location.root.clone(),
                                backend: Some(location.kind),
                            });
                            registry.write(&registry_path)?;
                        }
                        projects.insert(name, Arc::clone(&opened));
                        (opened, true)
                    }
                }
            }
        };
        if created {
            tracing::info!(project = %project.name(), root = %project.path.display(), "project registered");
            project.start(self.publisher.clone());
        }
        if let Some(abbreviation) = abbreviation {
            let author = author.unwrap_or_else(|| project.machine().clone());
            start_tasks(&project, abbreviation, &author)?;
        }
        Ok((project.view(), created))
    }

    pub fn deregister(&self, name: &str) -> Result<(), ProjectsError> {
        let registry_path = self.registry_path()?;
        let project = {
            let mut projects = self.write_projects();
            let mut registry = ProjectRegistry::read(&registry_path)?.unwrap_or_default();
            let listed = registry.remove(name).is_some();
            if !projects.contains_key(name) {
                // An entry the daemon could not open has no live project to remove; removing it
                // here spares the user an edit of the file the daemon owns.
                if !listed {
                    return Err(ProjectsError::NoSuchProject(name.to_owned()));
                }
                registry.write(&registry_path)?;
                tracing::info!(project = %name, "registry entry removed; it was not being served");
                return Ok(());
            }
            if listed {
                registry.write(&registry_path)?;
            }
            projects.remove(name).expect("checked above")
        };
        project.stop();
        tracing::info!(project = %name, "project removed");
        Ok(())
    }

    pub fn rename_project(&self, from: &str, to: &str) -> Result<ProjectView, ProjectsError> {
        let registry_path = self.registry_path()?;
        if !registry::is_usable_name(to) {
            return Err(ProjectsError::BadName(to.to_owned()));
        }
        let project = {
            let mut projects = self.write_projects();
            if !projects.contains_key(from) {
                return Err(ProjectsError::NoSuchProject(from.to_owned()));
            }
            if to != from && projects.contains_key(to) {
                return Err(ProjectsError::NameTaken(to.to_owned()));
            }
            let project = projects.get(from).expect("checked above").clone();
            let mut registry = ProjectRegistry::read(&registry_path)?.unwrap_or_default();
            if to != from && registry.holds_name(to) {
                return Err(ProjectsError::NameTaken(to.to_owned()));
            }
            let entry = ProjectEntry {
                name: to.to_owned(),
                path: project.path.clone(),
                backend: Some(project.kind()),
            };
            if !registry.replace(from, entry.clone()) {
                registry.insert(entry);
            }
            registry.write(&registry_path)?;
            projects.remove(from);
            project.rename(to);
            projects.insert(to.to_owned(), Arc::clone(&project));
            project
        };
        Ok(project.view())
    }

    pub fn stop(&self) {
        self.stop_for(StopReason::Stop);
    }

    // The first reason wins, so the stop an update starts still reads as an update when a signal
    // follows it.
    fn stop_for(&self, reason: StopReason) {
        self.shutdown.send_if_modified(|current| {
            let first = current.is_none();
            if first {
                *current = Some(reason);
            }
            first
        });
    }

    pub fn stop_reason(&self) -> Option<StopReason> {
        *self.shutdown.borrow()
    }

    // `install` runs and the daemon stops for the update only while no agent session is live. Both
    // happen under the lock that starting a session takes, so no session starts in between.
    pub fn update_if_idle<E>(&self, install: impl FnOnce() -> Result<(), E>) -> Result<bool, E> {
        let sessions = self.agents.held();
        if self.stop_reason().is_some() || sessions.values().any(|session| session.live()) {
            return Ok(false);
        }
        install()?;
        self.stop_for(StopReason::Update);
        Ok(true)
    }

    pub(crate) fn stopping(&self) -> watch::Receiver<Option<StopReason>> {
        self.shutdown.subscribe()
    }

    pub(crate) fn publisher(&self) -> &Publisher {
        &self.publisher
    }

    fn serving(&self, location: &Location) -> Option<Arc<Project>> {
        self.read_projects()
            .values()
            .find(|project| project.location().key() == location.key())
            .cloned()
    }

    fn registry_path(&self) -> Result<Arc<PathBuf>, ProjectsError> {
        self.registry.clone().ok_or(ProjectsError::NoRegistry)
    }

    fn read_projects(&self) -> std::sync::RwLockReadGuard<'_, BTreeMap<String, Arc<Project>>> {
        self.projects.read().expect("projects lock poisoned")
    }

    fn write_projects(&self) -> std::sync::RwLockWriteGuard<'_, BTreeMap<String, Arc<Project>>> {
        self.projects.write().expect("projects lock poisoned")
    }
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState").finish_non_exhaustive()
    }
}

fn project_name(
    registry: &ProjectRegistry,
    projects: &BTreeMap<String, Arc<Project>>,
    location: &Location,
) -> Result<String, ProjectsError> {
    let name = match registry.entry_at(&location.root) {
        Some(entry) => entry.name.clone(),
        None => registry::unique_name(&location.root, |name| {
            registry.holds_name(name) || projects.contains_key(name)
        }),
    };
    if !registry::is_usable_name(&name) {
        return Err(ProjectsError::BadName(name));
    }
    if projects.contains_key(&name) {
        return Err(ProjectsError::NameTaken(name));
    }
    Ok(name)
}

// A git project first takes what the remote already holds, so a second person starting the same
// repository joins its tasks rather than starting a rival set.
fn start_tasks(
    project: &Project,
    abbreviation: op_task::Abbreviation,
    author: &Actor,
) -> Result<(), ProjectsError> {
    let tracker = project.tracker();
    if !tracker.plan()?.is_initialized()
        && let Some(remote) = tracker.backend().remote()
        && let Err(err) = remote.sync()
    {
        tracing::warn!(project = %project.name(), error = %err, "starting the tasks without the remote");
    }
    tracker.init(author, abbreviation)?;
    project.reload();
    Ok(())
}

// A registered path that cannot be opened is skipped, not fatal: one broken checkout must not take
// the task UI away from every other project. A duplicate name, or a second entry for one set of
// tasks, is skipped for the same reason.
pub fn open_projects(entries: &[ProjectEntry]) -> Vec<Project> {
    let mut names = std::collections::BTreeSet::new();
    let mut keys = std::collections::BTreeSet::new();
    let mut projects = Vec::new();
    for entry in entries {
        if !registry::is_usable_name(&entry.name) {
            tracing::error!(project = %entry.name, path = %entry.path.display(), "skipping project: the name cannot address a project; use lowercase letters, digits, and dashes");
            continue;
        }
        if !names.insert(entry.name.clone()) {
            tracing::error!(project = %entry.name, "skipping project: the name is already taken");
            continue;
        }
        let opened = Location::find(&entry.path, entry.backend)
            .and_then(|location| Project::open(entry.name.clone(), location));
        match opened {
            Ok(project) => {
                if !keys.insert(project.location().key().to_path_buf()) {
                    tracing::error!(project = %entry.name, path = %entry.path.display(), "skipping project: another project already serves these tasks");
                    continue;
                }
                projects.push(project);
            }
            Err(err) => {
                tracing::error!(project = %entry.name, path = %entry.path.display(), error = format!("{err:#}"), "skipping project");
            }
        }
    }
    projects
}

#[derive(OpenApi)]
#[openapi(info(
    title = "openplan",
    description = "openplan daemon HTTP API",
    version = "0.1.0"
))]
struct ApiDoc;

fn documented() -> OpenApiRouter<AppState> {
    OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(health))
        .routes(routes!(list_projects, register_project))
        .routes(routes!(delete_project, rename_project))
        .routes(routes!(tasks::list_tasks, tasks::create_task))
        .routes(routes!(agent::list_sessions, agent::create_session))
        .routes(routes!(agent::delete_session))
        .routes(routes!(agent::prompt_session))
        .routes(routes!(agent::interrupt_session))
        .routes(routes!(agent::approve))
        .routes(routes!(tasks::get_board))
        .routes(routes!(tasks::get_merged_board))
        .routes(routes!(drawing::draw_flow))
        .routes(routes!(drawing::draw_diagram))
        .routes(routes!(tasks::search_project))
        .routes(routes!(tasks::search_all))
        .routes(routes!(
            tasks::get_task,
            tasks::patch_task,
            tasks::delete_task
        ))
        .routes(routes!(tasks::write_task_file))
        .routes(routes!(tasks::resolve_conflict))
        .routes(routes!(tasks::get_task_tree))
        .routes(routes!(tasks::list_comments, tasks::add_comment))
        .routes(routes!(tasks::task_history))
        .routes(routes!(tasks::task_revision))
        .routes(routes!(tasks::project_history))
        .routes(routes!(tasks::get_sync, tasks::run_sync))
        .routes(routes!(tasks::list_tags, tasks::create_tag))
        .routes(routes!(tasks::get_tag, tasks::patch_tag, tasks::delete_tag))
}

pub fn openapi() -> utoipa::openapi::OpenApi {
    let mut spec = documented().split_for_parts().1;
    if let Some(components) = spec.components.as_mut() {
        for schema in components.schemas.values_mut() {
            close_objects(schema);
        }
    }
    spec
}

// utoipa leaves `additionalProperties` unset, and JSON Schema reads that as "any extra field".
// The web client generator follows it and types every response as an open record.
fn close_objects(schema: &mut RefOr<Schema>) {
    let RefOr::T(schema) = schema else { return };
    match schema {
        Schema::Object(object) => {
            if !object.properties.is_empty() && object.additional_properties.is_none() {
                object.additional_properties =
                    Some(Box::new(AdditionalProperties::FreeForm(false)));
            }
            object.properties.values_mut().for_each(close_objects);
        }
        Schema::Array(array) => {
            if let ArrayItems::RefOrSchema(items) = &mut array.items {
                close_objects(items);
            }
        }
        Schema::OneOf(one_of) => one_of.items.iter_mut().for_each(close_objects),
        Schema::AllOf(all_of) => all_of.items.iter_mut().for_each(close_objects),
        Schema::AnyOf(any_of) => any_of.items.iter_mut().for_each(close_objects),
        _ => {}
    }
}

pub(crate) fn project_of(state: &AppState, name: &str) -> Result<Arc<Project>, ApiError> {
    let project = state.project(name).ok_or_else(|| {
        ApiError::new(
            StatusCode::NOT_FOUND,
            format!("no such project: {name}; {}", registered(state)),
        )
    })?;
    match project.blocked() {
        None => Ok(project),
        Some(reason) => Err(ApiError::unavailable(format!(
            "project {} is not being served: {reason}",
            project.name()
        ))),
    }
}

fn registered(state: &AppState) -> String {
    let names: Vec<String> = state
        .projects()
        .iter()
        .map(|project| project.name())
        .collect();
    match names.is_empty() {
        true => "no project is registered".to_owned(),
        false => format!("registered projects: {}", names.join(", ")),
    }
}

// The identity a write is signed with: what the caller sent, or the project's own identity when it
// sent none.
pub(crate) fn actor_of(headers: &HeaderMap, project: &Project) -> Actor {
    author_of(headers).unwrap_or_else(|| Actor {
        via: header_text(headers, op_api::AGENT_HEADER),
        ..project.machine().clone()
    })
}

fn author_of(headers: &HeaderMap) -> Option<Actor> {
    Some(Actor {
        name: header_text(headers, op_api::AUTHOR_HEADER)?,
        email: header_text(headers, op_api::EMAIL_HEADER),
        via: header_text(headers, op_api::AGENT_HEADER),
    })
}

fn header_text(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .and_then(op_api::decode_header)
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
}

pub fn app(state: AppState) -> Router {
    let (router, api) = documented().split_for_parts();
    router
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", api))
        .route("/api/events", get(events))
        .route(
            "/api/projects/{project}/agent/sessions/{id}/events",
            get(agent::session_events),
        )
        .route("/admin/shutdown", axum::routing::post(admin_shutdown))
        .fallback(static_handler)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &Request<axum::body::Body>| {
                    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
                    let request_id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
                    let route = request
                        .extensions()
                        .get::<MatchedPath>()
                        .map_or_else(|| request.uri().path(), MatchedPath::as_str);
                    // ERROR level so the span stays enabled at any RUST_LOG that shows failures.
                    tracing::error_span!(
                        "request",
                        request_id,
                        method = %request.method(),
                        path = %request.uri().path(),
                        route = %route,
                        error = tracing::field::Empty,
                    )
                })
                .on_request(())
                .on_response(|response: &Response, latency: Duration, _: &Span| {
                    // A 5xx is reported once by on_failure.
                    if response.status().is_server_error() {
                        return;
                    }
                    let status = response.status().as_u16();
                    let latency_ms = latency.as_millis();
                    if latency >= SLOW_REQUEST {
                        tracing::warn!(status, latency_ms, "slow request");
                    } else {
                        tracing::debug!(status, latency_ms, "request served");
                    }
                })
                .on_failure(
                    |failure: ServerErrorsFailureClass, latency: Duration, _: &Span| {
                        tracing::error!(
                            %failure,
                            latency_ms = latency.as_millis(),
                            "request failed"
                        );
                    },
                ),
        )
        .with_state(state)
}

pub async fn serve(
    listener: tokio::net::TcpListener,
    state: AppState,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> std::io::Result<()> {
    let stop = state.clone();
    let agents = state.agents();
    let watchdog = tokio::spawn(watch_projects(state.clone()));
    let stopping = state.clone();
    let result = axum::serve(listener, app(state))
        .with_graceful_shutdown(async move {
            let mut stopping = stop.stopping();
            tokio::select! {
                _ = shutdown => {}
                _ = stopping.wait_for(Option::is_some) => {}
            }
            // An external signal arrives here without touching the watch; publish it so open SSE
            // streams observe the stop and end.
            stop.stop();
        })
        .await;
    watchdog.abort();
    agents.finish().await;
    let _ = tokio::task::spawn_blocking(move || {
        for project in stopping.projects() {
            project.stop();
        }
    })
    .await;
    result
}

// Demotes a project whose root is gone and promotes it when the root comes back, and reads the
// writes other processes made to each project's tasks.
async fn watch_projects(state: AppState) {
    loop {
        tokio::time::sleep(project::ROOT_POLL).await;
        let projects = state.projects();
        let moved = tokio::task::spawn_blocking(move || {
            let mut moved = false;
            for project in &projects {
                moved |= project.poll();
            }
            moved
        })
        .await
        .unwrap_or(false);
        if moved {
            state.publisher.publish(ChangeEvent::ProjectsChanged, None);
        }
    }
}

#[utoipa::path(
    get,
    path = "/health",
    responses((status = 200, description = "Daemon info when running as a daemon, else \"ok\"", body = DaemonInfo))
)]
async fn health(State(state): State<AppState>) -> Response {
    match &state.health {
        Some(info) => Json(info.as_ref()).into_response(),
        None => (StatusCode::OK, "ok").into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/projects",
    responses((status = 200, description = "Every registered project, servable or not", body = Vec<ProjectView>))
)]
async fn list_projects(State(state): State<AppState>) -> Result<Json<Vec<ProjectView>>, ApiError> {
    let views = blocking(move || Ok(state.projects().iter().map(|p| p.view()).collect())).await?;
    Ok(Json(views))
}

#[utoipa::path(
    post,
    path = "/api/projects",
    request_body = RegisterProject,
    responses(
        (status = 201, description = "Registered", body = ProjectView),
        (status = 200, description = "Already registered", body = ProjectView),
        (status = 400, description = "The path holds no tasks, needs a migration, or the request is invalid", body = ApiErrorBody),
        (status = 409, description = "The project already uses another abbreviation", body = ApiErrorBody),
        (status = 503, description = "This daemon serves a fixed set of projects", body = ApiErrorBody)
    )
)]
async fn register_project(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<RegisterProject>,
) -> Result<Response, ApiError> {
    let registering = state.clone();
    let author = author_of(&headers);
    let (view, created) = blocking(move || Ok(registering.register_as(&body, author)?)).await?;
    if created {
        state.publisher.publish(ChangeEvent::ProjectsChanged, None);
    }
    let status = match created {
        true => StatusCode::CREATED,
        false => StatusCode::OK,
    };
    Ok((status, Json(view)).into_response())
}

// The tasks stay where they are: the daemon serves them, it does not own them.
#[utoipa::path(
    delete,
    path = "/api/projects/{project}",
    params(("project" = String, Path, description = "Project name")),
    responses(
        (status = 204, description = "Removed"),
        (status = 404, description = "No such project", body = ApiErrorBody),
        (status = 503, description = "This daemon serves a fixed set of projects", body = ApiErrorBody)
    )
)]
async fn delete_project(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> Result<StatusCode, ApiError> {
    let removing = state.clone();
    blocking(move || Ok(removing.deregister(&project)?)).await?;
    state.publisher.publish(ChangeEvent::ProjectsChanged, None);
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    patch,
    path = "/api/projects/{project}",
    params(("project" = String, Path, description = "Project name")),
    request_body = RenameProject,
    responses(
        (status = 200, description = "Renamed", body = ProjectView),
        (status = 400, description = "The new name cannot address a project", body = ApiErrorBody),
        (status = 404, description = "No such project", body = ApiErrorBody),
        (status = 409, description = "The new name is already taken", body = ApiErrorBody),
        (status = 503, description = "This daemon serves a fixed set of projects", body = ApiErrorBody)
    )
)]
async fn rename_project(
    State(state): State<AppState>,
    Path(project): Path<String>,
    Json(body): Json<RenameProject>,
) -> Result<Json<ProjectView>, ApiError> {
    let renaming = state.clone();
    let view = blocking(move || Ok(renaming.rename_project(&project, &body.name)?)).await?;
    state.publisher.publish(ChangeEvent::ProjectsChanged, None);
    Ok(Json(view))
}

async fn admin_shutdown(State(state): State<AppState>, headers: HeaderMap) -> Response {
    // A header a cross-site form POST cannot set without a preflight, so a page the user browses
    // cannot shut the daemon down.
    if !headers.contains_key(op_api::ADMIN_HEADER) {
        return StatusCode::FORBIDDEN.into_response();
    }
    state.stop();
    (StatusCode::OK, "shutting down").into_response()
}

// A new EventSource cannot set a header, so a page that opens one to reconnect sends its cursor in
// the query instead.
#[derive(serde::Deserialize)]
struct EventsQuery {
    last_event_id: Option<String>,
}

// A browser sends the id of the last event it saw when it reconnects, and the stream replays what
// came after it. A cursor this daemon cannot serve gets `Resync` instead.
async fn events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<EventsQuery>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let mut shutdown = state.stopping();
    let publisher = state.publisher.clone();
    let mut changes = BroadcastStream::new(publisher.subscribe());
    let cursor = headers
        .get(LAST_EVENT_ID)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
        .or(query.last_event_id);
    let catch_up = cursor.as_deref().map(|cursor| publisher.since(cursor));
    let (tx, rx) = mpsc::channel(EVENT_CHANNEL_CAPACITY);

    tokio::spawn(async move {
        let mut seen = 0;
        match catch_up {
            Some(Some(missed)) => {
                for published in missed {
                    seen = published.seq;
                    if tx.send(Ok(encode(&publisher, &published))).await.is_err() {
                        return;
                    }
                }
            }
            Some(None) => {
                let _ = tx.send(Ok(plain(&ChangeEvent::Resync))).await;
            }
            None => {}
        }
        loop {
            let event = tokio::select! {
                _ = tx.closed() => return,
                stopped = shutdown.wait_for(Option::is_some) => {
                    let reason = stopped.ok().and_then(|reason| *reason).unwrap_or(StopReason::Stop);
                    let _ = tx.try_send(Ok(plain(&ChangeEvent::DaemonStopping { reason })));
                    return;
                }
                message = changes.next() => match message {
                    Some(Ok(published)) if published.seq <= seen => continue,
                    Some(Ok(published)) => Ok(encode(&publisher, &published)),
                    Some(Err(BroadcastStreamRecvError::Lagged(_))) => Ok(plain(&ChangeEvent::Resync)),
                    None => return,
                },
            };
            tokio::select! {
                result = tx.send(event) => {
                    if result.is_err() {
                        return;
                    }
                }
                stopped = shutdown.wait_for(Option::is_some) => {
                    let reason = stopped.ok().and_then(|reason| *reason).unwrap_or(StopReason::Stop);
                    let _ = tx.try_send(Ok(plain(&ChangeEvent::DaemonStopping { reason })));
                    return;
                }
            }
        }
    });

    Sse::new(ReceiverStream::new(rx)).keep_alive(KeepAlive::default())
}

fn encode(publisher: &Publisher, published: &Published) -> Event {
    plain(&published.event).id(publisher.id(published.seq))
}

fn plain(change: &ChangeEvent) -> Event {
    Event::default()
        .json_data(change)
        .unwrap_or_else(|_| Event::default().comment("failed to encode change event"))
}

pub(crate) async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, ApiError> + Send + 'static,
) -> Result<T, ApiError> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(join_error)?
}

pub(crate) struct ApiError {
    status: StatusCode,
    message: String,
    reason: Option<Refusal>,
    cycles: Vec<Vec<String>>,
    position: Option<SourcePosition>,
}

impl ApiError {
    pub(crate) fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
            reason: None,
            cycles: Vec::new(),
            position: None,
        }
    }

    pub(crate) fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, message)
    }

    pub(crate) fn internal(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, message)
    }

    pub(crate) fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, message)
    }

    pub(crate) fn conflict(message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, message)
    }

    pub(crate) fn unavailable(message: impl Into<String>) -> Self {
        Self::new(StatusCode::SERVICE_UNAVAILABLE, message)
    }
}

impl From<TrackerError> for ApiError {
    fn from(err: TrackerError) -> Self {
        let status = match &err {
            TrackerError::NotFound { .. } | TrackerError::TagNotFound { .. } => {
                StatusCode::NOT_FOUND
            }
            TrackerError::Invalid(_)
            | TrackerError::InvalidRef { .. }
            | TrackerError::TagUnregistered { .. }
            | TrackerError::InvalidColor(_) => StatusCode::BAD_REQUEST,
            TrackerError::TagExists { .. }
            | TrackerError::TagReferenced { .. }
            | TrackerError::AlreadyInitialized(_)
            | TrackerError::NotInitialized
            | TrackerError::ConflictGone
            | TrackerError::Backend(BackendError::Contended) => StatusCode::CONFLICT,
            // The request is fine; the stored document is what has to change.
            TrackerError::MissingCreated { .. }
            | TrackerError::Unreadable { .. }
            | TrackerError::Config(_)
            | TrackerError::Task(_) => StatusCode::UNPROCESSABLE_ENTITY,
            TrackerError::Backend(BackendError::UnknownRevision(_)) => StatusCode::NOT_FOUND,
            TrackerError::Backend(BackendError::Sync(_)) => StatusCode::BAD_GATEWAY,
            TrackerError::Backend(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let reason = match &err {
            TrackerError::TagReferenced { .. } => Some(Refusal::TagReferenced),
            TrackerError::TagUnregistered { .. } => Some(Refusal::TagUnregistered),
            _ => None,
        };
        Self {
            reason,
            ..Self::new(status, err.to_string())
        }
    }
}

impl From<BackendError> for ApiError {
    fn from(err: BackendError) -> Self {
        TrackerError::Backend(err).into()
    }
}

// A cycle is the request's answer, not a fault of the daemon: the tasks name an order that cannot
// exist, and only an edit to them can change it.
impl From<FlowCycles> for ApiError {
    fn from(err: FlowCycles) -> Self {
        let message = err.to_string();
        Self {
            cycles: err.cycles,
            ..Self::new(StatusCode::UNPROCESSABLE_ENTITY, message)
        }
    }
}

// A diagram that does not parse is the answer to the request, as a cycle is: only an edit to the
// source can change it.
impl From<op_diagram_mermaid::ParseError> for ApiError {
    fn from(err: op_diagram_mermaid::ParseError) -> Self {
        Self {
            position: Some(SourcePosition {
                line: err.line,
                column: err.column,
            }),
            ..Self::new(StatusCode::UNPROCESSABLE_ENTITY, err.to_string())
        }
    }
}

impl From<KeyError> for ApiError {
    fn from(err: KeyError) -> Self {
        Self::bad_request(err.to_string())
    }
}

impl From<ProjectsError> for ApiError {
    fn from(err: ProjectsError) -> Self {
        let status = match &err {
            ProjectsError::Tracker(_) => return tracker_refusal(err),
            ProjectsError::Open(_)
            | ProjectsError::BadName(_)
            | ProjectsError::RelativePath(_)
            | ProjectsError::BadAbbreviation(_) => StatusCode::BAD_REQUEST,
            ProjectsError::NoSuchProject(_) => StatusCode::NOT_FOUND,
            ProjectsError::NameTaken(_) => StatusCode::CONFLICT,
            ProjectsError::NoRegistry => StatusCode::SERVICE_UNAVAILABLE,
            ProjectsError::Registry(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        Self::new(status, err.to_string())
    }
}

fn tracker_refusal(err: ProjectsError) -> ApiError {
    match err {
        ProjectsError::Tracker(err) => err.into(),
        other => ApiError::internal(other.to_string()),
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        if self.status.is_server_error() {
            Span::current().record("error", tracing::field::display(&self.message));
        }
        (
            self.status,
            Json(ApiErrorBody {
                message: self.message,
                reason: self.reason,
                cycles: self.cycles,
                position: self.position,
            }),
        )
            .into_response()
    }
}

pub(crate) fn join_error(err: tokio::task::JoinError) -> ApiError {
    ApiError::internal(format!("task failed: {err}"))
}

async fn static_handler(uri: Uri) -> Response {
    // Everything the SPA routes to falls back to index.html, so an API path no route matched must
    // refuse rather than answer with the page.
    if uri.path().starts_with("/api/") {
        return ApiError::not_found(format!("no such route: {}", uri.path())).into_response();
    }
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    match Assets::get(path) {
        Some(file) => {
            let mut response = (
                [(header::CONTENT_TYPE, content_type(path))],
                file.data.into_owned(),
            )
                .into_response();
            if path.starts_with(HASHED_ASSETS) {
                response
                    .headers_mut()
                    .insert(header::CACHE_CONTROL, IMMUTABLE_CACHE);
            }
            response
        }
        None => match Assets::get("index.html") {
            Some(file) => (
                [(header::CONTENT_TYPE, content_type("index.html"))],
                file.data.into_owned(),
            )
                .into_response(),
            None => StatusCode::NOT_FOUND.into_response(),
        },
    }
}

// Vite names every file under `assets/` by content hash, so the browser can keep one for as long
// as it likes.
const HASHED_ASSETS: &str = "assets/";
const IMMUTABLE_CACHE: HeaderValue =
    HeaderValue::from_static("public, max-age=31536000, immutable");

fn content_type(path: &str) -> &'static str {
    match path.rsplit_once('.').map(|(_, ext)| ext) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("wasm") => "application/wasm",
        _ => "application/octet-stream",
    }
}
