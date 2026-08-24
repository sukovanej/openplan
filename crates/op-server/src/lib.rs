use std::collections::BTreeMap;
use std::convert::Infallible;
use std::future::Future;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use axum::{
    Json, Router,
    extract::{MatchedPath, Path, Query, State},
    http::{HeaderMap, Request, StatusCode, Uri, header},
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
    routing::get,
};
use op_api::{
    Abbreviation, ApiErrorBody, Board, BranchState, ChangeEvent, ChangeKind, CreateTag, CreateTask,
    DaemonInfo, KeyError, Matrix, ProjectView, RegisterProject, RenameProject, SearchHit,
    StoreConfig, TagPatch, TagView, TaskBranches, TaskDetail, TaskListItem, TaskPatch, TaskTree,
    TaskTreeView, TaskView,
};
use op_git::Repo;
use op_index::{Index, IndexError};
use op_store::{Store, StoreError};
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, mpsc, watch};
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tokio_stream::wrappers::{BroadcastStream, ReceiverStream};
use tokio_stream::{Stream, StreamExt as _};
use tower_http::classify::ServerErrorsFailureClass;
use tower_http::trace::TraceLayer;
use tracing::Span;
use utoipa::{OpenApi, ToSchema};
use utoipa_axum::{router::OpenApiRouter, routes};
use utoipa_swagger_ui::SwaggerUi;

mod project;
mod registry;
pub use project::{OpenError, Project, open_projects, serve_root};
pub use registry::{ProjectEntry, ProjectRegistry, REGISTRY_FILE, RegistryError};

const EVENT_CHANNEL_CAPACITY: usize = 256;
const SLOW_REQUEST: Duration = Duration::from_millis(1000);
const ID_ATTEMPTS: usize = 16;

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
    // The daemon runs in its own home directory. A relative path would resolve there rather than
    // where the caller stands, and register whatever repository happens to sit at that spot — a
    // write that lands in a repository nobody named. Refuse it; only the caller can resolve it.
    #[error("a project path must be absolute, and {0} is not")]
    RelativePath(PathBuf),
}

#[derive(Clone)]
pub struct AppState {
    projects: Arc<RwLock<BTreeMap<String, Arc<Project>>>>,
    // Absent for a state built from a fixed project list, which has no file to keep in step: the
    // project routes serve it, and the ones that change membership refuse.
    registry: Option<Arc<PathBuf>>,
    shutdown: Arc<watch::Sender<bool>>,
    health: Option<Arc<DaemonInfo>>,
    events: broadcast::Sender<ChangeEvent>,
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
            shutdown: Arc::new(watch::channel(false).0),
            health: None,
            events: broadcast::channel(EVENT_CHANNEL_CAPACITY).0,
        }
    }

    pub fn with_registry(mut self, path: PathBuf) -> Self {
        self.registry = Some(Arc::new(path));
        self
    }

    pub fn with_health(mut self, info: DaemonInfo) -> Self {
        self.health = Some(Arc::new(info));
        self
    }

    pub fn event_sender(&self) -> broadcast::Sender<ChangeEvent> {
        self.events.clone()
    }

    // The read guard covers membership only, and is released before the caller touches the project.
    pub fn project(&self, name: &str) -> Option<Arc<Project>> {
        self.read_projects().get(name).cloned()
    }

    pub fn projects(&self) -> Vec<Arc<Project>> {
        self.read_projects().values().cloned().collect()
    }

    // Blocking: each watcher scans every branch and hashes each worktree's task files.
    pub fn start_watchers(&self) {
        for project in self.projects() {
            project::start_watch(&project, self.events.clone());
        }
    }

    // Idempotent by repository, not by the path asked for: the CLI auto-registers on its first write
    // and two of those can race, and a second worktree of a repository already served is that same
    // project. The bool says whether this call is what added it.
    pub fn register(&self, path: &std::path::Path) -> Result<(ProjectView, bool), ProjectsError> {
        if path.is_relative() {
            return Err(ProjectsError::RelativePath(path.to_path_buf()));
        }
        let registry_path = self.registry_path()?;
        let repo = Repo::discover(path).map_err(|_| OpenError::NoRepo(path.to_path_buf()))?;
        let root = registry::canonical(&serve_root(&repo, path));

        // The guard covers membership only. `Project::view` reads the abbreviation under the index
        // mutex, which a rebuild holds for a full branch walk, so it is taken after the guard drops:
        // holding the map's write lock across another project's rebuild is the cross-project
        // coupling every project having its own index mutex exists to avoid.
        let (project, created) = {
            let mut projects = self.write_projects();
            let common = project::git_common_dir(&repo);
            match projects
                .values()
                .find(|project| project.git_common_dir() == common)
            {
                Some(existing) => (Arc::clone(existing), false),
                None => {
                    let mut registry = ProjectRegistry::read(&registry_path)?.unwrap_or_default();
                    // A registered path that is not open was skipped at startup, or written by
                    // another daemon. Re-opening under the name it already carries keeps the file
                    // the one source of names — so a name the file cannot address is the file's to
                    // fix, and so is a name it already gave to a project being served.
                    let name = match registry.entry_at(&root) {
                        Some(entry) => entry.name.clone(),
                        None => registry::unique_name(&root, |name| {
                            registry.holds_name(name) || projects.contains_key(name)
                        }),
                    };
                    if !registry::is_usable_name(&name) {
                        return Err(ProjectsError::BadName(name));
                    }
                    if projects.contains_key(&name) {
                        return Err(ProjectsError::NameTaken(name));
                    }
                    // Open before the file is touched, so a path that cannot be served leaves no
                    // entry behind that every later start would have to skip.
                    let project = Arc::new(Project::open(name.clone(), root.clone())?);
                    if !registry.holds_name(&name) {
                        registry.insert(ProjectEntry {
                            name: name.clone(),
                            path: root,
                        });
                        registry.write(&registry_path)?;
                    }
                    projects.insert(name, Arc::clone(&project));
                    (project, true)
                }
            }
        };
        if created {
            tracing::info!(project = %project.name(), root = %project.path.display(), "project registered");
            project::start_watch(&project, self.events.clone());
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
                // An entry the daemon could not open at startup — a deleted checkout, a duplicate
                // name — has no live project to remove. Removing it here is the only way out that
                // does not ask the user to edit the file the daemon says it owns.
                if !listed {
                    return Err(ProjectsError::NoSuchProject(name.to_owned()));
                }
                registry.write(&registry_path)?;
                tracing::info!(project = %name, "registry entry removed; it was not being served");
                return Ok(());
            }
            if listed {
                // Before the map, so a write that fails leaves the daemon exactly as it was rather
                // than serving one membership while the file records another.
                registry.write(&registry_path)?;
            }
            projects.remove(name).expect("checked above")
        };
        // Outside the write lock: stopping joins the watcher's worker thread.
        project.stop_watch();
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
            // Written as the entry the live project is, rather than patched in place: the file and
            // the map can only disagree if one was edited by hand, and the served project is the
            // truth.
            let entry = ProjectEntry {
                name: to.to_owned(),
                path: project.path.clone(),
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
        let _ = self.shutdown.send(true);
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

#[derive(OpenApi)]
#[openapi(info(
    title = "open-planner",
    description = "open-planner daemon HTTP API",
    version = "0.1.0"
))]
struct ApiDoc;

fn documented() -> OpenApiRouter<AppState> {
    OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(health))
        .routes(routes!(list_projects, register_project))
        .routes(routes!(delete_project, rename_project))
        .routes(routes!(get_config))
        .routes(routes!(list_tasks, create_task))
        .routes(routes!(get_matrix))
        .routes(routes!(get_board))
        .routes(routes!(get_merged_board))
        .routes(routes!(search_project))
        .routes(routes!(search_all))
        .routes(routes!(get_task, patch_task, delete_task))
        .routes(routes!(get_task_tree))
        .routes(routes!(get_task_branches))
        .routes(routes!(list_tags, create_tag))
        .routes(routes!(get_tag, patch_tag, delete_tag))
}

pub fn openapi() -> utoipa::openapi::OpenApi {
    documented().split_for_parts().1
}

fn project_of(state: &AppState, name: &str) -> Result<Arc<Project>, ApiError> {
    let project = state.project(name).ok_or_else(|| {
        ApiError::new(
            StatusCode::NOT_FOUND,
            format!("no such project: {name}; {}", registered(state)),
        )
    })?;
    servable(project)
}

// A demoted project is still registered and still listed; it just cannot answer for its store. Its
// own routes say why, and every other project keeps answering.
fn servable(project: Arc<Project>) -> Result<Arc<Project>, ApiError> {
    match project.blocked() {
        None => Ok(project),
        Some(reason) => Err(ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            format!("project {} is not being served: {reason}", project.name()),
        )),
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

pub fn app(state: AppState) -> Router {
    let (router, api) = documented().split_for_parts();
    router
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", api))
        .route("/api/events", get(events))
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
                    // ERROR level so the span (and its route/id fields) stays enabled at any
                    // RUST_LOG that shows failures; `error` is filled in later on a 5xx.
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
                    // A 5xx is reported once by on_failure; skip it here to keep one line/request.
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
    let stop = state.shutdown.clone();
    let watchdog = tokio::spawn(watch_roots(state.clone()));
    let result = axum::serve(listener, app(state))
        .with_graceful_shutdown(async move {
            let mut stopping = stop.subscribe();
            tokio::select! {
                _ = shutdown => {}
                _ = stopping.wait_for(|&stopping| stopping) => {}
            }
            // An external signal arrives here without touching the watch; publish it so open SSE
            // streams observe the stop and end, instead of pinning graceful shutdown open.
            let _ = stop.send(true);
        })
        .await;
    watchdog.abort();
    result
}

// A checkout deleted under a running daemon leaves that project serving a tree that no longer
// exists: no write can land and its watch set is gone. It demotes that project alone — the daemon
// stops only on a signal or `/admin/shutdown` — and a root that comes back promotes it again.
async fn watch_roots(state: AppState) {
    loop {
        tokio::time::sleep(project::ROOT_POLL).await;
        let projects = state.projects();
        // `is_dir` on a dead network mount can block for as long as the mount's timeout. Every
        // project is polled on every tick, so no short-circuit here.
        let moved = tokio::task::spawn_blocking(move || {
            let mut moved = false;
            for project in &projects {
                moved |= project.poll_root();
            }
            moved
        })
        .await
        .unwrap_or(false);
        if moved {
            publish(&state, ChangeEvent::ProjectsChanged);
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
    // Each view reads its project's abbreviation under the index mutex, which a rebuild holds.
    let views = tokio::task::spawn_blocking(move || {
        state
            .projects()
            .iter()
            .map(|project| project.view())
            .collect()
    })
    .await
    .map_err(join_error)?;
    Ok(Json(views))
}

// Registration is by repository: the path is resolved to the checkout that can serve it, and a
// repository already served answers with the entry it already has. That is what lets the CLI
// register on its first write without coordinating with any other CLI doing the same.
#[utoipa::path(
    post,
    path = "/api/projects",
    request_body = RegisterProject,
    responses(
        (status = 201, description = "Registered", body = ProjectView),
        (status = 200, description = "Already registered", body = ProjectView),
        (status = 400, description = "The path has no task store, or no git repository", body = ApiErrorBody),
        (status = 503, description = "This daemon serves a fixed set of projects", body = ApiErrorBody)
    )
)]
async fn register_project(
    State(state): State<AppState>,
    Json(body): Json<RegisterProject>,
) -> Result<Response, ApiError> {
    let registering = state.clone();
    let path = PathBuf::from(body.path);
    let (view, created) = tokio::task::spawn_blocking(move || registering.register(&path))
        .await
        .map_err(join_error)??;
    if created {
        publish(&state, ChangeEvent::ProjectsChanged);
    }
    let status = match created {
        true => StatusCode::CREATED,
        false => StatusCode::OK,
    };
    Ok((status, Json(view)).into_response())
}

// Removal takes the project out of the registry and stops its watcher. The files on disk are left
// exactly as they are: the daemon serves a repository, it does not own one.
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
    tokio::task::spawn_blocking(move || removing.deregister(&project))
        .await
        .map_err(join_error)??;
    publish(&state, ChangeEvent::ProjectsChanged);
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
    let view = tokio::task::spawn_blocking(move || renaming.rename_project(&project, &body.name))
        .await
        .map_err(join_error)??;
    publish(&state, ChangeEvent::ProjectsChanged);
    Ok(Json(view))
}

// The abbreviation every key on the wire carries, so a client can tell this store's keys from any
// other spelling. It only ever changes with the store's `config.toml`, which announces itself over
// `/api/events`.
#[utoipa::path(
    get,
    path = "/api/projects/{project}/config",
    params(("project" = String, Path, description = "Project name")),
    responses(
        (status = 200, description = "The served store's configuration", body = StoreConfig),
        (status = 404, description = "No such project", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
async fn get_config(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> Result<Json<StoreConfig>, ApiError> {
    config_of(project_of(&state, &project)?).await
}

async fn config_of(project: Arc<Project>) -> Result<Json<StoreConfig>, ApiError> {
    let abbreviation = tokio::task::spawn_blocking(move || project.abbreviation())
        .await
        .map_err(join_error)?;
    Ok(Json(StoreConfig {
        abbreviation: abbreviation.to_string(),
    }))
}

async fn admin_shutdown(State(state): State<AppState>, headers: HeaderMap) -> Response {
    // Require a header a cross-site form POST cannot set without a preflight (which has no
    // CORS allowance here), so a page the user is browsing cannot drive-by shut us down.
    if !headers.contains_key(op_api::ADMIN_HEADER) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let _ = state.shutdown.send(true);
    (StatusCode::OK, "shutting down").into_response()
}

async fn events(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let mut shutdown = state.shutdown.subscribe();
    let mut changes = BroadcastStream::new(state.events.subscribe());
    let (tx, rx) = mpsc::channel(EVENT_CHANNEL_CAPACITY);

    // Race the broadcast against the shutdown watch and the client's own liveness so a stopping
    // daemon — or a vanished client — ends the stream at once instead of pinning graceful shutdown
    // open. The shutdown arm guards the send too: a client behind a full buffer must not park the
    // task inside `tx.send` where it can no longer observe the stop. The trailing DaemonStopping
    // lets the UI tell an intentional stop from a crash; a connection that subscribes mid-shutdown
    // sees the latched `true` and ends at once.
    tokio::spawn(async move {
        loop {
            let event = tokio::select! {
                _ = tx.closed() => return,
                _ = shutdown.wait_for(|&stopping| stopping) => {
                    let _ = tx.try_send(Ok(encode(&ChangeEvent::DaemonStopping)));
                    return;
                }
                message = changes.next() => match message {
                    Some(Ok(change)) => Ok(encode(&change)),
                    // A lagging client dropped events, and nothing says which; it refetches all of it.
                    Some(Err(BroadcastStreamRecvError::Lagged(_))) => Ok(encode(&ChangeEvent::Resync)),
                    None => return,
                },
            };
            tokio::select! {
                result = tx.send(event) => {
                    if result.is_err() {
                        return;
                    }
                }
                _ = shutdown.wait_for(|&stopping| stopping) => {
                    let _ = tx.try_send(Ok(encode(&ChangeEvent::DaemonStopping)));
                    return;
                }
            }
        }
    });

    Sse::new(ReceiverStream::new(rx)).keep_alive(KeepAlive::default())
}

fn encode(change: &ChangeEvent) -> Event {
    Event::default()
        .json_data(change)
        .unwrap_or_else(|_| Event::default().comment("failed to encode change event"))
}

fn publish(state: &AppState, event: ChangeEvent) {
    tracing::info!(?event, "change published");
    let _ = state.events.send(event);
}

#[derive(Serialize, ToSchema)]
struct CreatedTask {
    id: String,
}

struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, message)
    }

    fn internal(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, message)
    }

    // A write aimed at a branch no live worktree has checked out. The server never fabricates a
    // commit, so it refuses rather than retargeting silently.
    fn not_writable(branch: &str) -> Self {
        Self::new(
            StatusCode::CONFLICT,
            format!(
                "branch {branch} is not checked out in a writable worktree (no live worktree, or \
                 an operation is in progress); refusing to write"
            ),
        )
    }

    // The daemon's own root is gone, so no branch resolves and every write would read as
    // `not_writable` — a diagnosis that sends the user looking at their checkout instead.
    fn root_gone(root: &std::path::Path) -> Self {
        Self::new(
            StatusCode::CONFLICT,
            format!(
                "the daemon's root {} no longer exists, so it can no longer resolve any branch; \
                 restart it with `openplan server restart`",
                root.display()
            ),
        )
    }

    fn no_id_available(message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, message)
    }
}

impl From<StoreError> for ApiError {
    fn from(err: StoreError) -> Self {
        let status = match &err {
            StoreError::NotFound { .. } | StoreError::TagNotFound { .. } => StatusCode::NOT_FOUND,
            StoreError::Invalid(_)
            | StoreError::InvalidRef { .. }
            | StoreError::InvalidColor(_) => StatusCode::BAD_REQUEST,
            StoreError::TagExists { .. } | StoreError::TagReferenced { .. } => StatusCode::CONFLICT,
            // The request is fine and the daemon is fine; the stored file is the thing that has to
            // change, and only a human can change it.
            StoreError::MissingCreated { .. } | StoreError::TagFile { .. } => {
                StatusCode::UNPROCESSABLE_ENTITY
            }
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        Self::new(status, err.to_string())
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
            ProjectsError::Open(_) | ProjectsError::BadName(_) | ProjectsError::RelativePath(_) => {
                StatusCode::BAD_REQUEST
            }
            ProjectsError::NoSuchProject(_) => StatusCode::NOT_FOUND,
            ProjectsError::NameTaken(_) => StatusCode::CONFLICT,
            ProjectsError::NoRegistry => StatusCode::SERVICE_UNAVAILABLE,
            ProjectsError::Registry(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        Self::new(status, err.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        // TraceLayer's failure line only sees the status code; record the cause onto the request
        // span so its single ERROR line carries route + request id + why, without a second event.
        if self.status.is_server_error() {
            Span::current().record("error", tracing::field::display(&self.message));
        }
        (
            self.status,
            Json(ApiErrorBody {
                message: self.message,
            }),
        )
            .into_response()
    }
}

fn join_error(err: tokio::task::JoinError) -> ApiError {
    ApiError::internal(format!("task failed: {err}"))
}

// What every read route accepts. `branch` scopes the answer to one branch, and omitting it means
// whatever "no branch was named" means for that route. `fresh` is for a caller with no change
// stream: it pays a full rebuild rather than accept an answer the watcher has not caught up with.
#[derive(Deserialize, utoipa::IntoParams)]
struct ReadQuery {
    branch: Option<String>,
    #[serde(default)]
    fresh: bool,
}

#[derive(Deserialize, utoipa::IntoParams)]
struct TreeQuery {
    branch: Option<String>,
    #[serde(default)]
    fresh: bool,
    depth: Option<usize>,
}

// What a read that is not about one branch accepts.
#[derive(Deserialize, utoipa::IntoParams)]
struct FreshQuery {
    #[serde(default)]
    fresh: bool,
}

// What a search accepts. `q` is the literal the index tests every task's text against; an empty
// one matches nothing, so a caller may send every keystroke without a special case for the first.
#[derive(Deserialize, utoipa::IntoParams)]
struct SearchQuery {
    #[serde(default)]
    q: String,
    #[serde(default)]
    fresh: bool,
}

fn index_of(project: &Project, fresh: bool) -> Result<std::sync::MutexGuard<'_, Index>, ApiError> {
    match fresh {
        true => project.rebuilt_index(),
        false => project.read_index(),
    }
    .map_err(index_error)
}

// An unknown branch is refused rather than answered with an empty set: every branch the repository
// has is indexed, so "no cells here" and "no such branch" are different answers and only one of
// them is the caller's mistake.
fn known_branch(index: &Index, branch: &str) -> Result<(), ApiError> {
    if index.has_branch(branch) {
        return Ok(());
    }
    // The index walks `refs/heads/*`, so a repository whose first commit is still to come has no
    // branch at all — including the one HEAD points at. Denying that branch by name sends the
    // reader looking for a branch they are standing on; the commit is what is missing.
    // [[OPP-58]] is what makes these reads work before it exists.
    let message = match index.has_branches() {
        true => format!("no such branch: {branch}"),
        false => "this repository has no commits yet, so no branch holds any task; commit to read \
                  through the daemon"
            .to_owned(),
    };
    Err(ApiError::new(StatusCode::NOT_FOUND, message))
}

// The branch a read is scoped to when none was named: the serve-root worktree's own, mirroring the
// write path.
fn read_branch(index: &Index, requested: Option<String>) -> Result<String, ApiError> {
    let branch = match requested {
        Some(branch) => branch,
        None => index.current_branch().map(str::to_owned).ok_or_else(|| {
            ApiError::bad_request("cannot determine the current worktree's branch")
        })?,
    };
    known_branch(index, &branch)?;
    Ok(branch)
}

// One entry per logical task with every branch it lives on, or — with `?branch=` — the task set of
// that one branch alone, in the same shape. Like every handler it works inside `spawn_blocking`: the
// rebuild reads the object DB and the store waits on flocks, which must never stall an async worker
// thread.
#[utoipa::path(
    get,
    path = "/api/projects/{project}/tasks",
    params(("project" = String, Path, description = "Project name"), ReadQuery),
    responses(
        (status = 200, description = "Every logical task aggregated across branches, or one branch's own task set", body = Vec<TaskListItem>),
        (status = 400, description = "The request is invalid, or a stored task is malformed", body = ApiErrorBody),
        (status = 404, description = "No such project, or no such branch", body = ApiErrorBody),
        (status = 500, description = "The store or the repository could not be read", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
async fn list_tasks(
    State(state): State<AppState>,
    Path(project): Path<String>,
    Query(query): Query<ReadQuery>,
) -> Result<Json<Vec<TaskListItem>>, ApiError> {
    list_tasks_of(project_of(&state, &project)?, query).await
}

async fn list_tasks_of(
    project: Arc<Project>,
    query: ReadQuery,
) -> Result<Json<Vec<TaskListItem>>, ApiError> {
    let items = tokio::task::spawn_blocking(move || -> Result<Vec<TaskListItem>, ApiError> {
        let index = index_of(&project, query.fresh)?;
        Ok(match query.branch {
            Some(branch) => {
                known_branch(&index, &branch)?;
                index.branch_tasks(&project.name(), &branch)
            }
            None => index.aggregated_tasks(&project.name()),
        })
    })
    .await
    .map_err(join_error)??;
    Ok(Json(items))
}

// The task×branch matrix itself: one cell per branch that diverges from the default branch's
// merge-base. It is the only read that answers about branches rather than about tasks, so it is the
// only one with no `?branch=`.
#[utoipa::path(
    get,
    path = "/api/projects/{project}/matrix",
    params(("project" = String, Path, description = "Project name"), FreshQuery),
    responses(
        (status = 200, description = "One cell per task×branch that diverges from the merge-base", body = Matrix),
        (status = 404, description = "No such project", body = ApiErrorBody),
        (status = 500, description = "The store or the repository could not be read", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
async fn get_matrix(
    State(state): State<AppState>,
    Path(project): Path<String>,
    Query(query): Query<FreshQuery>,
) -> Result<Json<Matrix>, ApiError> {
    let project = project_of(&state, &project)?;
    let matrix = tokio::task::spawn_blocking(move || -> Result<Matrix, ApiError> {
        Ok(index_of(&project, query.fresh)?.matrix().clone())
    })
    .await
    .map_err(join_error)??;
    Ok(Json(matrix))
}

// The subtree rooted at one task, on one branch. `hierarchy_context` answers a different question —
// a parent title and the *direct* children a detail page embeds — so this walks the branch's whole
// task set instead.
#[utoipa::path(
    get,
    path = "/api/projects/{project}/tasks/{id}/tree",
    params(
        ("project" = String, Path, description = "Project name"),
        ("id" = String, Path, description = "Task key", pattern = "^[A-Z]{3}-(0|[1-9][0-9]*)$"),
        TreeQuery
    ),
    responses(
        (status = 200, description = "The subtree rooted at the task", body = TaskTreeView),
        (status = 400, description = "The request is invalid, or a stored task is malformed", body = ApiErrorBody),
        (status = 404, description = "No such project, no such branch, or no such task", body = ApiErrorBody),
        (status = 500, description = "The store or the repository could not be read", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
async fn get_task_tree(
    State(state): State<AppState>,
    Path((project, id)): Path<(String, String)>,
    Query(query): Query<TreeQuery>,
) -> Result<Json<TaskTreeView>, ApiError> {
    let project = project_of(&state, &project)?;
    let view = tokio::task::spawn_blocking(move || -> Result<TaskTreeView, ApiError> {
        let index = index_of(&project, query.fresh)?;
        reject_non_key(index.abbreviation(), &id)?;
        // Naming no branch means the same here as it does on the task itself: the version that
        // headlines. A tree built from another branch's task set would give the task a different
        // parent and different children than the detail read beside it.
        let branch = read_branch(&index, query.branch.or_else(|| index.headline_branch(&id)))?;
        let summaries = index.branch_summaries(&branch);
        let mut cycles = Vec::new();
        let tree = TaskTree::build(&summaries, &id, query.depth, &mut cycles)
            .ok_or_else(|| no_such_task(&index, &id, Some(&branch)))?;
        Ok(TaskTreeView {
            tree,
            cycles: first_seen(cycles),
        })
    })
    .await
    .map_err(join_error)??;
    Ok(Json(view))
}

fn first_seen(ids: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    ids.into_iter()
        .filter(|id| seen.insert(id.clone()))
        .collect()
}

// Which branches hold the task, and which of them hold the same bytes. Never branch-scoped: the
// question is about the spread across branches.
#[utoipa::path(
    get,
    path = "/api/projects/{project}/tasks/{id}/branches",
    params(
        ("project" = String, Path, description = "Project name"),
        ("id" = String, Path, description = "Task key", pattern = "^[A-Z]{3}-(0|[1-9][0-9]*)$"),
        FreshQuery
    ),
    responses(
        (status = 200, description = "Every version of the task, grouped by blob, with the branches holding it", body = TaskBranches),
        (status = 400, description = "The request is invalid, or a stored task is malformed", body = ApiErrorBody),
        (status = 404, description = "No such project, or the task is on no branch", body = ApiErrorBody),
        (status = 500, description = "The store or the repository could not be read", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
async fn get_task_branches(
    State(state): State<AppState>,
    Path((project, id)): Path<(String, String)>,
    Query(query): Query<FreshQuery>,
) -> Result<Json<TaskBranches>, ApiError> {
    let project = project_of(&state, &project)?;
    let branches = tokio::task::spawn_blocking(move || -> Result<TaskBranches, ApiError> {
        let index = index_of(&project, query.fresh)?;
        reject_non_key(index.abbreviation(), &id)?;
        index.task_branches(&id).ok_or_else(|| {
            ApiError::new(
                StatusCode::NOT_FOUND,
                format!("task not found on any branch: {id}"),
            )
        })
    })
    .await
    .map_err(join_error)??;
    Ok(Json(branches))
}

// The list view's whole data set in one read: tasks grouped by status and flattened into
// render-ordered rows. Built from the same branch-aware aggregation as `list_tasks`, so board
// reads stay "reads global"; the client consumes it verbatim.
#[utoipa::path(
    get,
    path = "/api/projects/{project}/board",
    params(("project" = String, Path, description = "Project name")),
    responses(
        (status = 200, description = "Every task grouped by status and flattened into render-ordered rows", body = Board),
        (status = 400, description = "The request is invalid, or a stored task is malformed", body = ApiErrorBody),
        (status = 404, description = "No such project", body = ApiErrorBody),
        (status = 500, description = "The store or the repository could not be read", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
async fn get_board(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> Result<Json<Board>, ApiError> {
    board_of(project_of(&state, &project)?).await
}

async fn board_of(project: Arc<Project>) -> Result<Json<Board>, ApiError> {
    let board = tokio::task::spawn_blocking(move || -> Result<Board, IndexError> {
        let tasks = project.read_index()?.aggregated_tasks(&project.name());
        Ok(Board::build(&tasks))
    })
    .await
    .map_err(join_error)?
    .map_err(index_error)?;
    Ok(Json(board))
}

// The board of every project at once, which is what the UI opens on. Each row carries its project,
// and `Board::build` keys on it, so two stores that commit the same abbreviation stay distinct here.
// A project that cannot answer contributes nothing and demotes itself, so the client learns why
// from `/api/projects` rather than reading a half-built board as the whole truth.
#[utoipa::path(
    get,
    path = "/api/board",
    responses(
        (status = 200, description = "Every task of every servable project, grouped by status and flattened into render-ordered rows", body = Board),
        (status = 500, description = "The store or the repository could not be read", body = ApiErrorBody)
    )
)]
async fn get_merged_board(State(state): State<AppState>) -> Result<Json<Board>, ApiError> {
    let board = tokio::task::spawn_blocking(move || {
        let mut tasks = Vec::new();
        for project in state.projects() {
            if let Some(reason) = project.blocked() {
                tracing::debug!(project = %project.name(), %reason, "project left off the merged board");
                continue;
            }
            // One project's index mutex at a time, never two: a rebuild holds it for a full branch
            // walk, and a board that nested them would make each project wait on all the others.
            // A project whose repository cannot be read right now drops off this board rather than
            // taking every other project's rows down with it. `read_index` records the failure, so
            // the project reports it on `/api/projects` instead of reading as empty.
            if let Ok(mut items) = project
                .read_index()
                .map(|index| index.aggregated_tasks(&project.name()))
            {
                tasks.append(&mut items);
            }
        }
        Board::build(&tasks)
    })
    .await
    .map_err(join_error)?;
    Ok(Json(board))
}

#[utoipa::path(
    get,
    path = "/api/projects/{project}/search",
    params(("project" = String, Path, description = "Project name"), SearchQuery),
    responses(
        (status = 200, description = "Every task of the project whose text contains the query, on any branch", body = Vec<SearchHit>),
        (status = 404, description = "No such project", body = ApiErrorBody),
        (status = 500, description = "The store or the repository could not be read", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
async fn search_project(
    State(state): State<AppState>,
    Path(project): Path<String>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Vec<SearchHit>>, ApiError> {
    let project = project_of(&state, &project)?;
    let hits = tokio::task::spawn_blocking(move || -> Result<Vec<SearchHit>, ApiError> {
        Ok(index_of(&project, query.fresh)?.search(&project.name(), &query.q))
    })
    .await
    .map_err(join_error)??;
    Ok(Json(hits))
}

// Search across every servable project, which is what the web palette asks: it opens over the
// merged board, so it searches the same set of tasks that board shows. A project that cannot answer
// drops out rather than failing the search, exactly as it drops off the merged board.
#[utoipa::path(
    get,
    path = "/api/search",
    params(SearchQuery),
    responses(
        (status = 200, description = "Every task of every servable project whose text contains the query, on any branch", body = Vec<SearchHit>),
        (status = 500, description = "The store or the repository could not be read", body = ApiErrorBody)
    )
)]
async fn search_all(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Vec<SearchHit>>, ApiError> {
    let hits = tokio::task::spawn_blocking(move || {
        let mut hits = Vec::new();
        for project in state.projects() {
            if project.blocked().is_some() {
                continue;
            }
            // One project's index mutex at a time, for the reason the merged board takes them one
            // at a time: a rebuild holds it for a full branch walk.
            if let Ok(mut found) =
                index_of(&project, query.fresh).map(|index| index.search(&project.name(), &query.q))
            {
                hits.append(&mut found);
            }
        }
        hits
    })
    .await
    .map_err(join_error)?;
    Ok(Json(hits))
}

// Creation is branch-local like every other write: it lands in the live worktree of the target
// branch or is refused. The id comes from the machine-wide counter above a floor taken across every
// local branch, so a number is issued once repo-wide and two branches can never mint different tasks
// under one id.
#[utoipa::path(
    post,
    path = "/api/projects/{project}/tasks",
    params(
        ("project" = String, Path, description = "Project name"),
        ("branch" = Option<String>, Query, description = "Branch to write; omit for the current worktree")
    ),
    request_body = CreateTask,
    responses(
        (status = 201, description = "Created", body = CreatedTask),
        (status = 400, description = "The task is invalid (unknown parent or dependency)", body = ApiErrorBody),
        (status = 404, description = "No such project", body = ApiErrorBody),
        (status = 409, description = "The write was refused: the branch is not checked out in a writable worktree, or the daemon's root is gone", body = ApiErrorBody),
        (status = 500, description = "The store or the repository could not be read", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
async fn create_task(
    State(state): State<AppState>,
    Path(project): Path<String>,
    Query(query): Query<BranchQuery>,
    Json(body): Json<CreateTask>,
) -> Result<Response, ApiError> {
    create_task_in(&state, project_of(&state, &project)?, query, body).await
}

async fn create_task_in(
    state: &AppState,
    project: Arc<Project>,
    query: BranchQuery,
    body: CreateTask,
) -> Result<Response, ApiError> {
    let writing = Arc::clone(&project);
    let created = op_task::now();
    let (branch, id) =
        tokio::task::spawn_blocking(move || -> Result<(String, String), ApiError> {
            let (branch, store, floor, abbreviation) = {
                let index = writing.rebuilt_index().map_err(index_error)?;
                let branch = write_branch(&index, query.branch)?;
                let (branch, store) = write_target(&index, writing.store(), branch)?;
                (
                    branch,
                    store,
                    index.max_id().unwrap_or(0),
                    index.abbreviation(),
                )
            };
            let task = body.into_task(created, abbreviation)?;
            let mut taken = 0;
            let number = loop {
                let number = writing.ids().issue_above(floor).ok_or_else(|| {
                    ApiError::no_id_available(
                        "the repository holds the highest task number there is; no number is left \
                         to issue",
                    )
                })?;
                match store.create(&task, number) {
                    Ok(number) => break number,
                    // The floor only covers ids the rebuild could see; a file written out of band since
                    // then can still hold the number, and the next one is free.
                    Err(StoreError::IdTaken { id }) if taken + 1 < ID_ATTEMPTS => {
                        taken += 1;
                        tracing::warn!(%id, "id already on disk; taking the next one");
                    }
                    Err(StoreError::IdTaken { id }) => {
                        return Err(ApiError::no_id_available(format!(
                            "{ID_ATTEMPTS} numbers in a row are already taken on disk, up to {id}; \
                             something outside the daemon is writing task files"
                        )));
                    }
                    Err(err) => return Err(err.into()),
                }
            };
            writing.mark_dirty();
            Ok((branch, abbreviation.format_key(number)))
        })
        .await
        .map_err(join_error)??;
    publish(
        state,
        ChangeEvent::TaskChanged {
            project: project.name(),
            id: id.clone(),
            branch,
        },
    );
    Ok((StatusCode::CREATED, Json(CreatedTask { id })).into_response())
}

#[derive(Deserialize)]
struct BranchQuery {
    branch: Option<String>,
}

// A `?branch=` reads that branch's version; omitting it returns the headline (current-worktree)
// version. Either way the response carries every branch the task lives on, resolved through the
// index so a task absent from the serve-root checkout still loads.
#[utoipa::path(
    get,
    path = "/api/projects/{project}/tasks/{id}",
    params(
        ("project" = String, Path, description = "Project name"),
        ("id" = String, Path, description = "Task key", pattern = "^[A-Z]{3}-(0|[1-9][0-9]*)$"),
        ReadQuery
    ),
    responses(
        (status = 200, description = "The task on the requested branch", body = TaskDetail),
        (status = 400, description = "The request is invalid, or a stored task is malformed", body = ApiErrorBody),
        (status = 404, description = "No such project, no such branch, or no such task", body = ApiErrorBody),
        (status = 500, description = "The store or the repository could not be read", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
async fn get_task(
    State(state): State<AppState>,
    Path((project, id)): Path<(String, String)>,
    Query(query): Query<ReadQuery>,
) -> Result<Json<TaskDetail>, ApiError> {
    get_task_of(project_of(&state, &project)?, id, query).await
}

async fn get_task_of(
    project: Arc<Project>,
    id: String,
    query: ReadQuery,
) -> Result<Json<TaskDetail>, ApiError> {
    let detail = tokio::task::spawn_blocking(move || -> Result<TaskDetail, ApiError> {
        let index = index_of(&project, query.fresh)?;
        // A read resolves through the index by matching cell ids, which would 404 a path segment no
        // key could ever be — where every write route answers 400. Refuse it here, once, for both.
        reject_non_key(index.abbreviation(), &id)?;
        if let Some(branch) = &query.branch {
            known_branch(&index, branch)?;
        }
        index
            .task_detail(
                project.repo(),
                &project.name(),
                &id,
                query.branch.as_deref(),
            )
            .map_err(index_error)?
            .ok_or_else(|| no_such_task(&index, &id, query.branch.as_deref()))
    })
    .await
    .map_err(join_error)??;
    Ok(Json(detail))
}

// A task the branch asked about does not carry may still exist on another one. Answering only "no
// such task" there sends the reader looking for a task the board still shows, so the refusal names
// the branches that do hold it. `branch` is the branch the request resolved to — named, so a caller
// whose branch the daemon picked reads which branch the answer is about.
fn no_such_task(index: &Index, id: &str, branch: Option<&str>) -> ApiError {
    let elsewhere: Vec<String> = index
        .task_branch_states(id)
        .iter()
        .filter(|state| state.kind != ChangeKind::Deleted)
        .map(branch_label)
        .collect();
    let message = match (elsewhere.is_empty(), branch) {
        (true, _) => format!("no such task: {id}"),
        (false, Some(branch)) => format!(
            "no such task on branch {branch}: {id}; it lives on {}",
            elsewhere.join(", ")
        ),
        (false, None) => format!(
            "no such task on this branch: {id}; it lives on {}",
            elsewhere.join(", ")
        ),
    };
    ApiError::new(StatusCode::NOT_FOUND, message)
}

fn branch_label(state: &BranchState) -> String {
    match state.dirty {
        true => format!("{} (dirty)", state.branch),
        false => state.branch.clone(),
    }
}

fn reject_non_key(abbreviation: Abbreviation, key: &str) -> Result<u64, ApiError> {
    abbreviation
        .parse_key(key)
        .ok_or_else(|| KeyError::new(abbreviation, key).into())
}

fn index_error(err: IndexError) -> ApiError {
    match err {
        IndexError::Store(err) => err.into(),
        IndexError::Git(err) => ApiError::internal(err.to_string()),
    }
}

// The write target: the branch requested via `?branch=`, else the serve-root worktree's own branch.
fn write_branch(index: &Index, requested: Option<String>) -> Result<String, ApiError> {
    match requested {
        Some(branch) => Ok(branch),
        None => index
            .current_branch()
            .map(str::to_owned)
            .ok_or_else(|| ApiError::bad_request("cannot determine the current worktree's branch")),
    }
}

// The write target of a write about one task. A named branch is an instruction and is taken as one,
// which is what keeps a CLI write on the branch its caller stands on. Naming none instead means
// "wherever this task lives": the branch the task headlines on when the serve root does not carry
// it. The aggregated reads offer tasks from every branch, so acting on one of those must not depend
// on which branch the serve root has checked out.
fn task_write_branch(
    index: &Index,
    id: &str,
    requested: Option<String>,
) -> Result<String, ApiError> {
    match requested {
        Some(branch) => Ok(branch),
        None => index
            .write_branch(id)
            .map(str::to_owned)
            .ok_or_else(|| ApiError::bad_request("cannot determine the current worktree's branch")),
    }
}

// Callers hold the index mutex across this and release it before writing, so the store's flock wait
// never blocks the reads that share the mutex. The index they hold comes from `Project::rebuilt_index`,
// which rebuilds in all conditions: the question here is which branch is writable right now.
fn write_target(
    index: &Index,
    serve_store: &Store,
    branch: String,
) -> Result<(String, Store), ApiError> {
    let store = index.live_store(&branch).ok_or_else(|| {
        if serve_store.root().is_dir() {
            ApiError::not_writable(&branch)
        } else {
            ApiError::root_gone(serve_store.root())
        }
    })?;
    Ok((branch, store))
}

// A write the store found no task for. The write resolved to one branch, and the reads that offered
// the task name every branch it lives on, so the refusal names them too rather than reading as "this
// task does not exist" about a task the board still shows.
fn write_not_found(project: &Project, branch: &str, id: &str, err: StoreError) -> ApiError {
    match (&err, project.read_index()) {
        (StoreError::NotFound { .. }, Ok(index)) => no_such_task(&index, id, Some(branch)),
        _ => err.into(),
    }
}

#[utoipa::path(
    patch,
    path = "/api/projects/{project}/tasks/{id}",
    params(
        ("project" = String, Path, description = "Project name"),
        ("id" = String, Path, description = "Task key", pattern = "^[A-Z]{3}-(0|[1-9][0-9]*)$"),
        ("branch" = Option<String>, Query, description = "Branch to write; omit to write wherever the task lives")
    ),
    request_body = TaskPatch,
    responses(
        (status = 200, description = "The updated task", body = TaskDetail),
        (status = 400, description = "The patch is invalid (unknown parent, or a parent cycle)", body = ApiErrorBody),
        (status = 404, description = "No such project, or no such task", body = ApiErrorBody),
        (status = 409, description = "The write was refused: the branch is not checked out in a writable worktree, or the daemon's root is gone", body = ApiErrorBody),
        (status = 500, description = "The store or the repository could not be read", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
async fn patch_task(
    State(state): State<AppState>,
    Path((project, id)): Path<(String, String)>,
    Query(query): Query<BranchQuery>,
    Json(patch): Json<TaskPatch>,
) -> Result<Json<TaskDetail>, ApiError> {
    patch_task_of(&state, project_of(&state, &project)?, id, query, patch).await
}

async fn patch_task_of(
    state: &AppState,
    project: Arc<Project>,
    id: String,
    query: BranchQuery,
    patch: TaskPatch,
) -> Result<Json<TaskDetail>, ApiError> {
    let writing = Arc::clone(&project);
    let (branch, detail) =
        tokio::task::spawn_blocking(move || -> Result<(String, TaskDetail), ApiError> {
            let (branch, store, abbreviation) = {
                let index = writing.rebuilt_index().map_err(index_error)?;
                let branch = task_write_branch(&index, &id, query.branch)?;
                let (branch, store) = write_target(&index, writing.store(), branch)?;
                (branch, store, index.abbreviation())
            };
            let number = reject_non_key(abbreviation, &id)?;
            // The refusal has to leave the closure, not be carried out of it: `update` writes
            // whenever the mutation reports success, and a patch that fails on its third field has
            // already changed the first two.
            let task = store
                .update(number, |task| {
                    patch
                        .apply(task, abbreviation)
                        .map_err(|err| StoreError::Invalid(err.to_string()))
                })
                .map_err(|err| write_not_found(&writing, &branch, &id, err))?;
            writing.mark_dirty();
            // Echo the freshly written task rather than re-reading it from the matrix: a write that
            // lands the branch back on its merge-base leaves no divergence cell, so a matrix lookup
            // would 404 a write that in fact succeeded — and its `updated` falls back to the
            // headline, the commit that already holds this exact content.
            let (headline, branches, write_target, parent_title, children, refs, updated) = {
                let index = writing.rebuilt_index().map_err(index_error)?;
                // The index resolves a parent by key; the frontmatter holds the number the file
                // layer names it by.
                let parent = task
                    .frontmatter
                    .parent
                    .as_deref()
                    .and_then(|parent| abbreviation.format_ref(parent));
                let (parent_title, children, refs) =
                    index.hierarchy_context(&writing.name(), &id, parent.as_deref(), &task.body);
                (
                    index.headline_branch(&id).unwrap_or_default(),
                    index.task_branch_states(&id),
                    index.write_target(&id, Some(&branch)),
                    parent_title,
                    children,
                    refs,
                    index.task_updated_or_headline(&id, Some(&branch)),
                )
            };
            let view = TaskView::from_task(id, &task, updated, abbreviation);
            let detail = TaskDetail {
                project: writing.name(),
                id: view.id,
                title: view.title,
                metadata: view.metadata,
                body: view.body,
                updated: view.updated,
                headline,
                branches,
                write_target,
                parent_title,
                children,
                refs,
            };
            Ok((branch, detail))
        })
        .await
        .map_err(join_error)??;
    publish(
        state,
        ChangeEvent::TaskChanged {
            project: project.name(),
            id: detail.id.clone(),
            branch,
        },
    );
    Ok(Json(detail))
}

#[utoipa::path(
    delete,
    path = "/api/projects/{project}/tasks/{id}",
    params(
        ("project" = String, Path, description = "Project name"),
        ("id" = String, Path, description = "Task key", pattern = "^[A-Z]{3}-(0|[1-9][0-9]*)$"),
        ("branch" = Option<String>, Query, description = "Branch to write; omit to write wherever the task lives")
    ),
    responses(
        (status = 204, description = "Deleted"),
        (status = 400, description = "The request is invalid, or a stored task is malformed", body = ApiErrorBody),
        (status = 404, description = "No such project, or no such task", body = ApiErrorBody),
        (status = 409, description = "The write was refused: the branch is not checked out in a writable worktree, or the daemon's root is gone", body = ApiErrorBody),
        (status = 500, description = "The store or the repository could not be read", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
async fn delete_task(
    State(state): State<AppState>,
    Path((project, id)): Path<(String, String)>,
    Query(query): Query<BranchQuery>,
) -> Result<StatusCode, ApiError> {
    delete_task_of(&state, project_of(&state, &project)?, id, query).await
}

async fn delete_task_of(
    state: &AppState,
    project: Arc<Project>,
    id: String,
    query: BranchQuery,
) -> Result<StatusCode, ApiError> {
    let writing = Arc::clone(&project);
    let changed = id.clone();
    let branch = tokio::task::spawn_blocking(move || -> Result<String, ApiError> {
        let (branch, store, abbreviation) = {
            let index = writing.rebuilt_index().map_err(index_error)?;
            let branch = task_write_branch(&index, &id, query.branch)?;
            let (branch, store) = write_target(&index, writing.store(), branch)?;
            (branch, store, index.abbreviation())
        };
        store
            .delete(reject_non_key(abbreviation, &id)?)
            .map_err(|err| write_not_found(&writing, &branch, &id, err))?;
        writing.mark_dirty();
        Ok(branch)
    })
    .await
    .map_err(join_error)??;
    publish(
        state,
        ChangeEvent::TaskChanged {
            project: project.name(),
            id: changed,
            branch,
        },
    );
    Ok(StatusCode::NO_CONTENT)
}

// A tag is a file in one worktree's `.plan/tags`, and the daemon keeps no cross-branch index of
// them, so a read resolves the same live worktree a write does: `?branch=` names it, and omitting
// it means the worktree the daemon serves.
fn tag_target(project: &Project, branch: Option<String>) -> Result<(String, Store), ApiError> {
    let index = project.rebuilt_index().map_err(index_error)?;
    let branch = write_branch(&index, branch)?;
    write_target(&index, project.store(), branch)
}

const TAG_NAME_PARAM: &str = "Tag name, or any spelling that normalizes to one";

#[utoipa::path(
    get,
    path = "/api/projects/{project}/tags",
    params(
        ("project" = String, Path, description = "Project name"),
        ("branch" = Option<String>, Query, description = "Branch to read; omit for the current worktree")
    ),
    responses(
        (status = 200, description = "Every tag the branch registers", body = Vec<TagView>),
        (status = 400, description = "The current worktree is on no branch", body = ApiErrorBody),
        (status = 404, description = "No such project", body = ApiErrorBody),
        (status = 409, description = "The branch is not checked out in a live worktree, or the daemon's root is gone", body = ApiErrorBody),
        (status = 422, description = "A tag file is stored in a form the daemon cannot read", body = ApiErrorBody),
        (status = 500, description = "The registry could not be read", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
async fn list_tags(
    State(state): State<AppState>,
    Path(project): Path<String>,
    Query(query): Query<BranchQuery>,
) -> Result<Json<Vec<TagView>>, ApiError> {
    let project = project_of(&state, &project)?;
    let tags = tokio::task::spawn_blocking(move || -> Result<Vec<TagView>, ApiError> {
        let (_, store) = tag_target(&project, query.branch)?;
        Ok(store.list_tags()?.iter().map(TagView::from).collect())
    })
    .await
    .map_err(join_error)??;
    Ok(Json(tags))
}

#[utoipa::path(
    post,
    path = "/api/projects/{project}/tags",
    params(
        ("project" = String, Path, description = "Project name"),
        ("branch" = Option<String>, Query, description = "Branch to write; omit for the current worktree")
    ),
    request_body = CreateTag,
    responses(
        (status = 201, description = "Registered", body = TagView),
        (status = 400, description = "The name cannot be normalized to a tag name", body = ApiErrorBody),
        (status = 404, description = "No such project", body = ApiErrorBody),
        (status = 409, description = "The name is already registered, the branch is not checked out in a writable worktree, or the daemon's root is gone", body = ApiErrorBody),
        (status = 422, description = "The color is not a palette name", body = ApiErrorBody),
        (status = 500, description = "The registry could not be written", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
async fn create_tag(
    State(state): State<AppState>,
    Path(project): Path<String>,
    Query(query): Query<BranchQuery>,
    Json(body): Json<CreateTag>,
) -> Result<Response, ApiError> {
    let project = project_of(&state, &project)?;
    let writing = Arc::clone(&project);
    let (branch, view) =
        tokio::task::spawn_blocking(move || -> Result<(String, TagView), ApiError> {
            let (branch, store) = tag_target(&writing, query.branch)?;
            let tag = body
                .into_tag()
                .map_err(|err| ApiError::bad_request(err.to_string()))?;
            store.create_tag(&tag)?;
            Ok((branch, TagView::from(&tag)))
        })
        .await
        .map_err(join_error)??;
    publish(
        &state,
        ChangeEvent::TagsChanged {
            project: project.name(),
            branch,
        },
    );
    Ok((StatusCode::CREATED, Json(view)).into_response())
}

#[utoipa::path(
    get,
    path = "/api/projects/{project}/tags/{name}",
    params(
        ("project" = String, Path, description = "Project name"),
        ("name" = String, Path, description = TAG_NAME_PARAM),
        ("branch" = Option<String>, Query, description = "Branch to read; omit for the current worktree")
    ),
    responses(
        (status = 200, description = "The tag", body = TagView),
        (status = 400, description = "The name cannot be normalized to a tag name", body = ApiErrorBody),
        (status = 404, description = "No such project, or no such tag", body = ApiErrorBody),
        (status = 409, description = "The branch is not checked out in a live worktree, or the daemon's root is gone", body = ApiErrorBody),
        (status = 422, description = "The tag file is stored in a form the daemon cannot read", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
async fn get_tag(
    State(state): State<AppState>,
    Path((project, name)): Path<(String, String)>,
    Query(query): Query<BranchQuery>,
) -> Result<Json<TagView>, ApiError> {
    let project = project_of(&state, &project)?;
    let view = tokio::task::spawn_blocking(move || -> Result<TagView, ApiError> {
        let (_, store) = tag_target(&project, query.branch)?;
        Ok(TagView::from(&store.read_tag(&name)?))
    })
    .await
    .map_err(join_error)??;
    Ok(Json(view))
}

// A rename moves the file and rewrites the `tags:` of every task on this branch that holds the old
// name, so those tasks change too and are published one by one.
#[utoipa::path(
    patch,
    path = "/api/projects/{project}/tags/{name}",
    params(
        ("project" = String, Path, description = "Project name"),
        ("name" = String, Path, description = TAG_NAME_PARAM),
        ("branch" = Option<String>, Query, description = "Branch to write; omit for the current worktree")
    ),
    request_body = TagPatch,
    responses(
        (status = 200, description = "The updated tag", body = TagView),
        (status = 400, description = "The new name cannot be normalized to a tag name", body = ApiErrorBody),
        (status = 404, description = "No such project, or no such tag", body = ApiErrorBody),
        (status = 409, description = "The new name is already registered, the branch is not checked out in a writable worktree, or the daemon's root is gone", body = ApiErrorBody),
        (status = 422, description = "The color is not a palette name, or the tag file is stored in a form the daemon cannot read", body = ApiErrorBody),
        (status = 500, description = "The registry could not be written", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
async fn patch_tag(
    State(state): State<AppState>,
    Path((project, name)): Path<(String, String)>,
    Query(query): Query<BranchQuery>,
    Json(patch): Json<TagPatch>,
) -> Result<Json<TagView>, ApiError> {
    let project = project_of(&state, &project)?;
    let writing = Arc::clone(&project);
    let (branch, view, retagged) = tokio::task::spawn_blocking(
        move || -> Result<(String, TagView, Vec<String>), ApiError> {
            let (branch, store) = tag_target(&writing, query.branch)?;
            // The rename goes first. It is the step that can refuse — on a name already taken, or
            // on a referencing task it cannot write — and refusing there leaves the tag untouched.
            let (name, retagged) = match &patch.name {
                Some(display) => {
                    let retagged = store.rename_tag(&name, display)?;
                    let renamed = op_task::tag::normalize_name(display)
                        .map_err(|err| ApiError::bad_request(err.to_string()))?;
                    (renamed, retagged)
                }
                None => (name, Vec::new()),
            };
            // A rename already wrote the file. Writing it again for a patch that changes nothing
            // inside it opens a window where a concurrent delete makes this report a failure the
            // rename has in fact carried out.
            let tag = match patch.changes_content() {
                true => store.update_tag(&name, |tag| {
                    patch.apply(tag);
                    Ok(())
                })?,
                false => store.read_tag(&name)?,
            };
            let abbreviation = writing.abbreviation();
            if !retagged.is_empty() {
                writing.mark_dirty();
            }
            let retagged = retagged
                .iter()
                .map(|number| abbreviation.format_key(*number))
                .collect();
            Ok((branch, TagView::from(&tag), retagged))
        },
    )
    .await
    .map_err(join_error)??;
    for id in retagged {
        publish(
            &state,
            ChangeEvent::TaskChanged {
                project: project.name(),
                id,
                branch: branch.clone(),
            },
        );
    }
    publish(
        &state,
        ChangeEvent::TagsChanged {
            project: project.name(),
            branch,
        },
    );
    Ok(Json(view))
}

#[utoipa::path(
    delete,
    path = "/api/projects/{project}/tags/{name}",
    params(
        ("project" = String, Path, description = "Project name"),
        ("name" = String, Path, description = TAG_NAME_PARAM),
        ("branch" = Option<String>, Query, description = "Branch to write; omit for the current worktree"),
        ("force" = Option<bool>, Query, description = "Delete the tag even while tasks on this branch reference it")
    ),
    responses(
        (status = 204, description = "Deleted"),
        (status = 400, description = "The name cannot be normalized to a tag name", body = ApiErrorBody),
        (status = 404, description = "No such project, or no such tag", body = ApiErrorBody),
        (status = 409, description = "Tasks on this branch reference the tag, the branch is not checked out in a writable worktree, or the daemon's root is gone", body = ApiErrorBody),
        (status = 500, description = "The registry could not be written", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
async fn delete_tag(
    State(state): State<AppState>,
    Path((project, name)): Path<(String, String)>,
    Query(query): Query<DeleteTagQuery>,
) -> Result<StatusCode, ApiError> {
    let project = project_of(&state, &project)?;
    let writing = Arc::clone(&project);
    let branch = tokio::task::spawn_blocking(move || -> Result<String, ApiError> {
        let (branch, store) = tag_target(&writing, query.branch)?;
        store.delete_tag(&name, query.force)?;
        Ok(branch)
    })
    .await
    .map_err(join_error)??;
    publish(
        &state,
        ChangeEvent::TagsChanged {
            project: project.name(),
            branch,
        },
    );
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct DeleteTagQuery {
    branch: Option<String>,
    #[serde(default)]
    force: bool,
}

async fn static_handler(uri: Uri) -> Response {
    // Everything the SPA routes to falls back to index.html, so an API path no route matched would
    // otherwise be answered with the page rather than with a refusal — a 200 of HTML where the
    // caller asked for JSON. A spelling this daemon has dropped has to say so.
    if uri.path().starts_with("/api/") {
        return ApiError::new(
            StatusCode::NOT_FOUND,
            format!("no such route: {}", uri.path()),
        )
        .into_response();
    }
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    match Assets::get(path) {
        Some(file) => (
            [(header::CONTENT_TYPE, content_type(path))],
            file.data.into_owned(),
        )
            .into_response(),
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
