use std::future::Future;
use std::sync::{Arc, Mutex};

use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, Uri, header},
    response::{IntoResponse, Response},
    routing::get,
};
use op_api::{CreateTask, DaemonInfo, Matrix, TaskPatch, TaskSummary, TaskView};
use op_index::Index;
use op_presence::Registry;
use op_store::{Store, StoreError};
use rust_embed::RustEmbed;
use serde::Serialize;
use tokio::sync::Notify;
use tower_http::trace::TraceLayer;

#[derive(RustEmbed)]
#[folder = "../../web"]
struct Assets;

#[derive(Clone)]
pub struct AppState {
    pub index: Arc<Mutex<Index>>,
    pub presence: Arc<Mutex<Registry>>,
    store: Option<Store>,
    shutdown: Arc<Notify>,
    health: Option<Arc<DaemonInfo>>,
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
            shutdown: Arc::new(Notify::new()),
            health: None,
        }
    }
}

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/matrix", get(matrix))
        .route("/api/tasks", get(list_tasks).post(create_task))
        .route(
            "/api/tasks/{id}",
            get(get_task).patch(patch_task).delete(delete_task),
        )
        .route("/admin/shutdown", axum::routing::post(admin_shutdown))
        .fallback(static_handler)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

pub async fn serve(
    listener: tokio::net::TcpListener,
    state: AppState,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> std::io::Result<()> {
    let admin = state.shutdown.clone();
    axum::serve(listener, app(state))
        .with_graceful_shutdown(async move {
            tokio::select! {
                _ = shutdown => {}
                _ = admin.notified() => {}
            }
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
    state.shutdown.notify_one();
    (StatusCode::OK, "shutting down").into_response()
}

async fn matrix(State(state): State<AppState>) -> Json<Matrix> {
    let index = state.index.lock().expect("index mutex poisoned");
    Json(index.matrix().clone())
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

async fn list_tasks(State(state): State<AppState>) -> Result<Json<Vec<TaskSummary>>, ApiError> {
    let store = require_store(&state)?;
    let summaries = blocking(move || {
        let mut summaries = Vec::new();
        for id in store.task_ids()? {
            match store.read(&id) {
                Ok(task) => summaries.push(TaskSummary::from_task(id, &task)),
                Err(err) => tracing::warn!(%id, %err, "skipping unreadable task"),
            }
        }
        Ok(summaries)
    })
    .await?;
    Ok(Json(summaries))
}

async fn create_task(
    State(state): State<AppState>,
    Json(body): Json<CreateTask>,
) -> Result<Response, ApiError> {
    let store = require_store(&state)?;
    let id = blocking(move || store.create(&body.into_task())).await?;
    tracing::info!(%id, "task changed");
    Ok((StatusCode::CREATED, Json(CreatedTask { id })).into_response())
}

async fn get_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<TaskView>, ApiError> {
    let store = require_store(&state)?;
    let view = blocking(move || {
        let task = store.read(&id)?;
        Ok(TaskView::from_task(id, &task))
    })
    .await?;
    Ok(Json(view))
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
        tracing::info!(%id, "task changed");
        Ok(TaskView::from_task(id, &task))
    })
    .await?;
    Ok(Json(view))
}

async fn delete_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let store = require_store(&state)?;
    blocking(move || {
        store.delete(&id)?;
        tracing::info!(%id, "task changed");
        Ok(())
    })
    .await?;
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
