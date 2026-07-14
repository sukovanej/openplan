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
use op_api::{ChangeEvent, CreateTask, DaemonInfo, TaskListItem, TaskPatch, TaskView};
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

const EVENT_CHANNEL_CAPACITY: usize = 256;
const SLOW_REQUEST: Duration = Duration::from_millis(1000);

#[derive(RustEmbed)]
#[folder = "../../web/packages/app/dist"]
struct Assets;

#[derive(Clone)]
pub struct AppState {
    pub index: Arc<Mutex<Index>>,
    pub presence: Arc<Mutex<Registry>>,
    store: Option<Store>,
    repo: Option<Repo>,
    shutdown: Arc<watch::Sender<bool>>,
    health: Option<Arc<DaemonInfo>>,
    events: broadcast::Sender<ChangeEvent>,
}

impl AppState {
    pub fn with_health(mut self, info: DaemonInfo) -> Self {
        self.health = Some(Arc::new(info));
        self
    }

    pub fn with_store(mut self, store: Store) -> Self {
        self.store = Some(store);
        self
    }

    pub fn with_repo(mut self, repo: Repo) -> Self {
        self.repo = Some(repo);
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

impl Default for AppState {
    fn default() -> Self {
        Self {
            index: Arc::new(Mutex::new(Index::new())),
            presence: Arc::new(Mutex::new(Registry::new())),
            store: None,
            repo: None,
            shutdown: Arc::new(watch::channel(false).0),
            health: None,
            events: broadcast::channel(EVENT_CHANNEL_CAPACITY).0,
        }
    }
}

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/events", get(events))
        .route("/api/tasks", get(list_tasks).post(create_task))
        .route(
            "/api/tasks/{id}",
            get(get_task).patch(patch_task).delete(delete_task),
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

#[derive(Serialize)]
struct CreatedTask {
    id: String,
}

struct ApiError(StoreError);

impl From<StoreError> for ApiError {
    fn from(err: StoreError) -> Self {
        Self(err)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match &self.0 {
            StoreError::NotFound { .. } => StatusCode::NOT_FOUND,
            StoreError::Invalid(_) => StatusCode::BAD_REQUEST,
            StoreError::StoreMissing | StoreError::Io(_) | StoreError::Task(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        // TraceLayer's failure line only sees the status code; record the cause onto the request
        // span so its single ERROR line carries route + request id + why, without a second event.
        if status.is_server_error() {
            Span::current().record("error", tracing::field::display(&self.0));
        }
        (status, self.0.to_string()).into_response()
    }
}

fn require_store(state: &AppState) -> Result<Store, ApiError> {
    state
        .store
        .clone()
        .ok_or(ApiError(StoreError::StoreMissing))
}

// The store does blocking file I/O and flock waits; run it off the async worker threads so a
// lock held elsewhere (e.g. a CLI write) can never stall the runtime.
async fn blocking<T>(
    f: impl FnOnce() -> Result<T, StoreError> + Send + 'static,
) -> Result<T, ApiError>
where
    T: Send + 'static,
{
    match tokio::task::spawn_blocking(f).await {
        Ok(result) => result.map_err(ApiError),
        Err(err) => Err(ApiError(StoreError::Io(std::io::Error::other(format!(
            "store task failed: {err}"
        ))))),
    }
}

// Branch-aware: rebuild the index from the serve root's repo + worktrees, then return one entry
// per logical task with every branch it lives on. Without a git repo, degrade to the current
// worktree's tasks (no branch awareness) so a non-git .plan store still lists.
async fn list_tasks(State(state): State<AppState>) -> Result<Json<Vec<TaskListItem>>, ApiError> {
    if let (Some(repo), Some(store)) = (state.repo.clone(), state.store.clone()) {
        let index = state.index.clone();
        let items =
            tokio::task::spawn_blocking(move || -> Result<Vec<TaskListItem>, IndexError> {
                let mut index = index.lock().expect("index mutex poisoned");
                index.rebuild(&repo, &store)?;
                Ok(index.aggregated_tasks())
            })
            .await
            .map_err(|err| {
                ApiError(StoreError::Io(std::io::Error::other(format!(
                    "index task failed: {err}"
                ))))
            })?
            .map_err(index_error)?;
        return Ok(Json(items));
    }
    let store = require_store(&state)?;
    let items = blocking(move || {
        let mut items = Vec::new();
        for id in store.task_ids()? {
            match store.read(&id) {
                Ok(task) => items.push(TaskListItem::from_task(id, &task)),
                Err(err) => tracing::warn!(%id, %err, "skipping unreadable task"),
            }
        }
        Ok(items)
    })
    .await?;
    Ok(Json(items))
}

async fn create_task(
    State(state): State<AppState>,
    Json(body): Json<CreateTask>,
) -> Result<Response, ApiError> {
    let store = require_store(&state)?;
    let id = blocking(move || store.create(&body.into_task())).await?;
    publish(
        &state,
        ChangeEvent::TaskChanged {
            id: id.clone(),
            branch: String::new(),
        },
    );
    Ok((StatusCode::CREATED, Json(CreatedTask { id })).into_response())
}

#[derive(Deserialize)]
struct TaskQuery {
    branch: Option<String>,
}

async fn get_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<TaskQuery>,
) -> Result<Json<TaskView>, ApiError> {
    // A `?branch=` reads that branch's version cross-branch (§7.1); omitting it keeps the local
    // current-worktree read the UI already relies on.
    if let Some(branch) = query.branch {
        return cross_branch_get(state, id, branch).await;
    }
    let store = require_store(&state)?;
    let view = blocking(move || {
        let task = store.read(&id)?;
        Ok(TaskView::from_task(id, &task))
    })
    .await?;
    Ok(Json(view))
}

async fn cross_branch_get(
    state: AppState,
    id: String,
    branch: String,
) -> Result<Json<TaskView>, ApiError> {
    let (Some(repo), Some(store)) = (state.repo.clone(), state.store.clone()) else {
        return Err(ApiError(StoreError::NotFound { id }));
    };
    let index = state.index.clone();
    let missing = id.clone();
    let view = tokio::task::spawn_blocking(move || -> Result<Option<TaskView>, ApiError> {
        let mut index = index.lock().expect("index mutex poisoned");
        index.rebuild(&repo, &store).map_err(index_error)?;
        index
            .effective_view(&repo, &id, &branch)
            .map_err(index_error)
    })
    .await
    .map_err(|err| {
        ApiError(StoreError::Io(std::io::Error::other(format!(
            "index task failed: {err}"
        ))))
    })??;
    view.map(Json)
        .ok_or(ApiError(StoreError::NotFound { id: missing }))
}

fn index_error(err: IndexError) -> ApiError {
    match err {
        IndexError::Store(err) => ApiError(err),
        IndexError::Git(err) => ApiError(StoreError::Io(std::io::Error::other(err.to_string()))),
    }
}

async fn patch_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(patch): Json<TaskPatch>,
) -> Result<Json<TaskView>, ApiError> {
    let store = require_store(&state)?;
    let view = blocking(move || {
        let task = store.update(&id, |task| {
            patch.apply(task);
            Ok(())
        })?;
        Ok(TaskView::from_task(id, &task))
    })
    .await?;
    publish(
        &state,
        ChangeEvent::TaskChanged {
            id: view.id.clone(),
            branch: String::new(),
        },
    );
    Ok(Json(view))
}

async fn delete_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let store = require_store(&state)?;
    let changed = id.clone();
    blocking(move || store.delete(&id)).await?;
    publish(
        &state,
        ChangeEvent::TaskChanged {
            id: changed,
            branch: String::new(),
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
