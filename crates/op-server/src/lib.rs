use std::future::Future;
use std::sync::{Arc, Mutex};

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode, Uri, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use op_api::{DaemonInfo, Matrix};
use op_index::Index;
use op_presence::Registry;
use rust_embed::RustEmbed;
use tokio::sync::Notify;
use tower_http::trace::TraceLayer;

#[derive(RustEmbed)]
#[folder = "../../web"]
struct Assets;

#[derive(Clone)]
pub struct AppState {
    pub index: Arc<Mutex<Index>>,
    pub presence: Arc<Mutex<Registry>>,
    shutdown: Arc<Notify>,
    health: Option<Arc<DaemonInfo>>,
}

impl AppState {
    pub fn with_health(mut self, info: DaemonInfo) -> Self {
        self.health = Some(Arc::new(info));
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
            shutdown: Arc::new(Notify::new()),
            health: None,
        }
    }
}

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/matrix", get(matrix))
        .route("/admin/shutdown", post(admin_shutdown))
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
