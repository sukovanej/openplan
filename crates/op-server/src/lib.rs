use std::convert::Infallible;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
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
    ApiErrorBody, Board, ChangeEvent, CreateTask, DaemonInfo, TaskDetail, TaskListItem, TaskPatch,
    TaskView,
};
use op_git::Repo;
use op_index::{Index, IndexError};
use op_presence::Registry;
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

const EVENT_CHANNEL_CAPACITY: usize = 256;
const SLOW_REQUEST: Duration = Duration::from_millis(1000);
const ID_ATTEMPTS: usize = 16;

#[derive(RustEmbed)]
#[folder = "../../web/packages/app/dist"]
struct Assets;

// The one place task ids are handed out. Handlers release the index mutex before their flock write,
// so two concurrent creates would each compute the same `max + 1` from the matrix; this counter is
// bumped atomically instead, and re-seeded from the floor on every call so a merge bringing in
// higher ids — or a restart — advances it. Nothing is persisted: the ids on disk are the floor.
#[derive(Debug, Default)]
struct IdCounter(Mutex<u64>);

impl IdCounter {
    // `u64::MAX` is never issued, only used as the exhausted marker: any id already carrying it
    // (hand-written, or merged in) would otherwise pin the counter there and hand the same number
    // to every later create. Losing one number of 2^64 keeps "issued at most once" unconditional.
    fn issue_above(&self, floor: u64) -> Option<u64> {
        let mut next = self.0.lock().expect("id counter mutex poisoned");
        let issued = (*next).max(floor.saturating_add(1));
        if issued == u64::MAX {
            *next = u64::MAX;
            return None;
        }
        *next = issued + 1;
        Some(issued)
    }
}

#[derive(Clone)]
pub struct AppState {
    pub index: Arc<Mutex<Index>>,
    pub presence: Arc<Mutex<Registry>>,
    store: Store,
    repo: Repo,
    ids: Arc<IdCounter>,
    shutdown: Arc<watch::Sender<bool>>,
    health: Option<Arc<DaemonInfo>>,
    events: broadcast::Sender<ChangeEvent>,
}

impl AppState {
    pub fn new(repo: Repo, store: Store) -> Self {
        Self {
            index: Arc::new(Mutex::new(Index::new())),
            presence: Arc::new(Mutex::new(Registry::new())),
            store,
            repo,
            ids: Arc::new(IdCounter::default()),
            shutdown: Arc::new(watch::channel(false).0),
            health: None,
            events: broadcast::channel(EVENT_CHANNEL_CAPACITY).0,
        }
    }

    pub fn with_health(mut self, info: DaemonInfo) -> Self {
        self.health = Some(Arc::new(info));
        self
    }

    pub fn event_sender(&self) -> broadcast::Sender<ChangeEvent> {
        self.events.clone()
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
        .routes(routes!(list_tasks, create_task))
        .routes(routes!(get_board))
        .routes(routes!(get_task, patch_task, delete_task))
}

pub fn openapi() -> utoipa::openapi::OpenApi {
    documented().split_for_parts().1
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
    axum::serve(listener, app(state))
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
        .await
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
                    // A lagging client dropped events; nudge it to refetch the whole list.
                    Some(Err(BroadcastStreamRecvError::Lagged(_))) => Ok(encode(&ChangeEvent::RefMoved {
                        branch: String::new(),
                    })),
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

enum ApiError {
    Store(StoreError),
    // A write aimed at a branch no live worktree has checked out. The server never fabricates a
    // commit, so it refuses rather than retargeting silently.
    NotWritable(String),
    // The daemon's own root is gone, so no branch resolves and every write would read as
    // `NotWritable` — a diagnosis that sends the user looking at their checkout instead.
    RootGone(std::path::PathBuf),
    NoIdAvailable(String),
}

impl From<StoreError> for ApiError {
    fn from(err: StoreError) -> Self {
        Self::Store(err)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::Store(StoreError::NotFound { id }) => {
                (StatusCode::NOT_FOUND, format!("no such task: {id}"))
            }
            ApiError::Store(StoreError::Invalid(message)) => (StatusCode::BAD_REQUEST, message),
            // The request is fine and the daemon is fine; the stored file is the thing that has to
            // change, and only a human can change it.
            ApiError::Store(err @ StoreError::MissingCreated { .. }) => {
                (StatusCode::UNPROCESSABLE_ENTITY, err.to_string())
            }
            ApiError::Store(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
            ApiError::NotWritable(branch) => (
                StatusCode::CONFLICT,
                format!(
                    "branch {branch} is not checked out in a writable worktree (no live worktree, \
                     or an operation is in progress); refusing to write"
                ),
            ),
            ApiError::RootGone(root) => (
                StatusCode::CONFLICT,
                format!(
                    "the daemon's root {} no longer exists, so it can no longer resolve any \
                     branch; restart it with `oplan server restart`",
                    root.display()
                ),
            ),
            ApiError::NoIdAvailable(message) => (StatusCode::CONFLICT, message),
        };
        // TraceLayer's failure line only sees the status code; record the cause onto the request
        // span so its single ERROR line carries route + request id + why, without a second event.
        if status.is_server_error() {
            Span::current().record("error", tracing::field::display(&message));
        }
        (status, Json(ApiErrorBody { message })).into_response()
    }
}

fn join_error(err: tokio::task::JoinError) -> ApiError {
    ApiError::Store(StoreError::Io(std::io::Error::other(format!(
        "task failed: {err}"
    ))))
}

// Rebuild the index from the serve root's repo + worktrees, then return one entry per logical task
// with every branch it lives on. Like every handler it works inside `spawn_blocking`: the rebuild
// reads the object DB and the store waits on flocks, which must never stall an async worker thread.
#[utoipa::path(
    get,
    path = "/api/tasks",
    responses(
        (status = 200, description = "Every logical task, aggregated across branches", body = Vec<TaskListItem>),
        (status = 400, description = "The request is invalid, or a stored task is malformed", body = ApiErrorBody),
        (status = 500, description = "The store or the repository could not be read", body = ApiErrorBody)
    )
)]
async fn list_tasks(State(state): State<AppState>) -> Result<Json<Vec<TaskListItem>>, ApiError> {
    let repo = state.repo.clone();
    let store = state.store.clone();
    let index = state.index.clone();
    let items = tokio::task::spawn_blocking(move || -> Result<Vec<TaskListItem>, IndexError> {
        let mut index = index.lock().expect("index mutex poisoned");
        index.rebuild(&repo, &store)?;
        Ok(index.aggregated_tasks())
    })
    .await
    .map_err(join_error)?
    .map_err(index_error)?;
    Ok(Json(items))
}

// The list view's whole data set in one read: tasks grouped by status and flattened into
// render-ordered rows (§9). Built from the same branch-aware aggregation as `list_tasks`, so board
// reads stay "reads global"; the client consumes it verbatim.
#[utoipa::path(
    get,
    path = "/api/board",
    responses(
        (status = 200, description = "Every task grouped by status and flattened into render-ordered rows", body = Board),
        (status = 400, description = "The request is invalid, or a stored task is malformed", body = ApiErrorBody),
        (status = 500, description = "The store or the repository could not be read", body = ApiErrorBody)
    )
)]
async fn get_board(State(state): State<AppState>) -> Result<Json<Board>, ApiError> {
    let repo = state.repo.clone();
    let store = state.store.clone();
    let index = state.index.clone();
    let board = tokio::task::spawn_blocking(move || -> Result<Board, IndexError> {
        let mut index = index.lock().expect("index mutex poisoned");
        index.rebuild(&repo, &store)?;
        Ok(Board::build(&index.aggregated_tasks()))
    })
    .await
    .map_err(join_error)?
    .map_err(index_error)?;
    Ok(Json(board))
}

// Creation is branch-local like every other write: it lands in the live worktree of the target
// branch or is refused. The id number comes from the machine-wide counter above a floor taken across
// every local branch, so a number is issued once repo-wide and two branches can never mint different
// tasks under one id.
#[utoipa::path(
    post,
    path = "/api/tasks",
    params(("branch" = Option<String>, Query, description = "Branch to write; omit for the current worktree")),
    request_body = CreateTask,
    responses(
        (status = 201, description = "Created", body = CreatedTask),
        (status = 400, description = "The task is invalid (unknown parent or dependency)", body = ApiErrorBody),
        (status = 409, description = "The write was refused: the branch is not checked out in a writable worktree, or the daemon's root is gone", body = ApiErrorBody),
        (status = 500, description = "The store or the repository could not be read", body = ApiErrorBody)
    )
)]
async fn create_task(
    State(state): State<AppState>,
    Query(query): Query<TaskQuery>,
    Json(body): Json<CreateTask>,
) -> Result<Response, ApiError> {
    let repo = state.repo.clone();
    let serve_store = state.store.clone();
    let index = state.index.clone();
    let ids = state.ids.clone();
    let created = op_task::now();
    let (branch, id) =
        tokio::task::spawn_blocking(move || -> Result<(String, String), ApiError> {
            let (branch, store, floor) = {
                let mut index = index.lock().expect("index mutex poisoned");
                let (branch, store) = write_target(&mut index, &repo, &serve_store, query.branch)?;
                (branch, store, index.max_id_number().unwrap_or(0))
            };
            let task = body.into_task(created);
            let mut taken = 0;
            let id = loop {
                let number = ids.issue_above(floor).ok_or_else(|| {
                    ApiError::NoIdAvailable(
                        "the repository holds the highest task number there is; no number is left \
                         to issue"
                            .to_owned(),
                    )
                })?;
                match store.create(&task, number) {
                    Ok(id) => break id,
                    // The floor only covers ids the rebuild could see; a file written out of band since
                    // then can still hold the number, and the next one is free.
                    Err(StoreError::IdTaken { id }) if taken + 1 < ID_ATTEMPTS => {
                        taken += 1;
                        tracing::warn!(%id, "id already on disk; taking the next one");
                    }
                    Err(StoreError::IdTaken { id }) => {
                        return Err(ApiError::NoIdAvailable(format!(
                            "{ID_ATTEMPTS} numbers in a row are already taken on disk, up to {id}; \
                             something outside the daemon is writing task files"
                        )));
                    }
                    Err(err) => return Err(ApiError::Store(err)),
                }
            };
            Ok((branch, id))
        })
        .await
        .map_err(join_error)??;
    publish(
        &state,
        ChangeEvent::TaskChanged {
            id: id.clone(),
            branch,
        },
    );
    Ok((StatusCode::CREATED, Json(CreatedTask { id })).into_response())
}

#[derive(Deserialize)]
struct TaskQuery {
    branch: Option<String>,
}

// A `?branch=` reads that branch's version; omitting it returns the headline (current-worktree)
// version. Either way the response carries every branch the task lives on, resolved through the
// index so a task absent from the serve-root checkout still loads.
#[utoipa::path(
    get,
    path = "/api/tasks/{id}",
    params(
        ("id" = String, Path, description = "Task id"),
        ("branch" = Option<String>, Query, description = "Branch version to read; omit for the headline")
    ),
    responses(
        (status = 200, description = "The task on the requested branch", body = TaskDetail),
        (status = 400, description = "The request is invalid, or a stored task is malformed", body = ApiErrorBody),
        (status = 404, description = "No such task", body = ApiErrorBody),
        (status = 500, description = "The store or the repository could not be read", body = ApiErrorBody)
    )
)]
async fn get_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<TaskQuery>,
) -> Result<Json<TaskDetail>, ApiError> {
    let repo = state.repo.clone();
    let store = state.store.clone();
    let index = state.index.clone();
    let missing = id.clone();
    let detail = tokio::task::spawn_blocking(move || -> Result<Option<TaskDetail>, ApiError> {
        let mut index = index.lock().expect("index mutex poisoned");
        index.rebuild(&repo, &store).map_err(index_error)?;
        index
            .task_detail(&repo, &id, query.branch.as_deref())
            .map_err(index_error)
    })
    .await
    .map_err(join_error)??;
    detail
        .map(Json)
        .ok_or(ApiError::Store(StoreError::NotFound { id: missing }))
}

fn index_error(err: IndexError) -> ApiError {
    match err {
        IndexError::Store(err) => ApiError::Store(err),
        IndexError::Git(err) => {
            ApiError::Store(StoreError::Io(std::io::Error::other(err.to_string())))
        }
    }
}

// The write target: the branch requested via `?branch=`, else the serve-root worktree's own branch.
fn write_branch(index: &Index, requested: Option<String>) -> Result<String, ApiError> {
    match requested {
        Some(branch) => Ok(branch),
        None => index.current_branch().map(str::to_owned).ok_or_else(|| {
            ApiError::Store(StoreError::Invalid(
                "cannot determine the current worktree's branch".to_owned(),
            ))
        }),
    }
}

// Callers hold the index mutex across this and release it before writing, so the store's flock wait
// never blocks the reads that share the mutex.
fn write_target(
    index: &mut Index,
    repo: &Repo,
    serve_store: &Store,
    requested: Option<String>,
) -> Result<(String, Store), ApiError> {
    index.rebuild(repo, serve_store).map_err(index_error)?;
    let branch = write_branch(index, requested)?;
    let store = index.live_store(&branch).ok_or_else(|| {
        if serve_store.root().is_dir() {
            ApiError::NotWritable(branch.clone())
        } else {
            ApiError::RootGone(serve_store.root().to_path_buf())
        }
    })?;
    Ok((branch, store))
}

#[utoipa::path(
    patch,
    path = "/api/tasks/{id}",
    params(
        ("id" = String, Path, description = "Task id"),
        ("branch" = Option<String>, Query, description = "Branch to write; omit for the current worktree")
    ),
    request_body = TaskPatch,
    responses(
        (status = 200, description = "The updated task", body = TaskDetail),
        (status = 400, description = "The patch is invalid (unknown parent, or a parent cycle)", body = ApiErrorBody),
        (status = 404, description = "No such task", body = ApiErrorBody),
        (status = 409, description = "The write was refused: the branch is not checked out in a writable worktree, or the daemon's root is gone", body = ApiErrorBody),
        (status = 500, description = "The store or the repository could not be read", body = ApiErrorBody)
    )
)]
async fn patch_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<TaskQuery>,
    Json(patch): Json<TaskPatch>,
) -> Result<Json<TaskDetail>, ApiError> {
    let repo = state.repo.clone();
    let serve_store = state.store.clone();
    let index = state.index.clone();
    let (branch, detail) =
        tokio::task::spawn_blocking(move || -> Result<(String, TaskDetail), ApiError> {
            let (branch, store) = {
                let mut index = index.lock().expect("index mutex poisoned");
                write_target(&mut index, &repo, &serve_store, query.branch)?
            };
            let task = store.update(&id, |task| {
                patch.apply(task);
                Ok(())
            })?;
            // Echo the freshly written task rather than re-reading it from the matrix: a write that
            // lands the branch back on its merge-base leaves no divergence cell, so a matrix lookup
            // would 404 a write that in fact succeeded — and its `updated` falls back to the
            // headline, the commit that already holds this exact content.
            let (headline, branches, parent_title, children, refs, updated) = {
                let mut index = index.lock().expect("index mutex poisoned");
                index.rebuild(&repo, &serve_store).map_err(index_error)?;
                let (parent_title, children, refs) =
                    index.hierarchy_context(&id, task.frontmatter.parent.as_deref(), &task.body);
                (
                    index.headline_branch(&id).unwrap_or_default(),
                    index.task_branch_states(&id),
                    parent_title,
                    children,
                    refs,
                    index.task_updated_or_headline(&id, Some(&branch)),
                )
            };
            let view = TaskView::from_task(id, &task, updated);
            let detail = TaskDetail {
                id: view.id,
                title: view.title,
                metadata: view.metadata,
                body: view.body,
                updated: view.updated,
                headline,
                branches,
                parent_title,
                children,
                refs,
            };
            Ok((branch, detail))
        })
        .await
        .map_err(join_error)??;
    publish(
        &state,
        ChangeEvent::TaskChanged {
            id: detail.id.clone(),
            branch,
        },
    );
    Ok(Json(detail))
}

#[utoipa::path(
    delete,
    path = "/api/tasks/{id}",
    params(
        ("id" = String, Path, description = "Task id"),
        ("branch" = Option<String>, Query, description = "Branch to write; omit for the current worktree")
    ),
    responses(
        (status = 204, description = "Deleted"),
        (status = 400, description = "The request is invalid, or a stored task is malformed", body = ApiErrorBody),
        (status = 404, description = "No such task", body = ApiErrorBody),
        (status = 409, description = "The write was refused: the branch is not checked out in a writable worktree, or the daemon's root is gone", body = ApiErrorBody),
        (status = 500, description = "The store or the repository could not be read", body = ApiErrorBody)
    )
)]
async fn delete_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<TaskQuery>,
) -> Result<StatusCode, ApiError> {
    let repo = state.repo.clone();
    let serve_store = state.store.clone();
    let index = state.index.clone();
    let changed = id.clone();
    let branch = tokio::task::spawn_blocking(move || -> Result<String, ApiError> {
        let (branch, store) = {
            let mut index = index.lock().expect("index mutex poisoned");
            write_target(&mut index, &repo, &serve_store, query.branch)?
        };
        store.delete(&id)?;
        Ok(branch)
    })
    .await
    .map_err(join_error)??;
    publish(
        &state,
        ChangeEvent::TaskChanged {
            id: changed,
            branch,
        },
    );
    Ok(StatusCode::NO_CONTENT)
}

async fn static_handler(uri: Uri) -> Response {
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
