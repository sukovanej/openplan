use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::Request;
use op_server::{AppState, app};
use tower::ServiceExt as _;
use tracing::instrument::WithSubscriber as _;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::MakeWriter;

#[derive(Clone, Default)]
struct Buffer(Arc<Mutex<Vec<u8>>>);

impl Buffer {
    fn contents(&self) -> String {
        String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
    }
}

impl std::io::Write for Buffer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for Buffer {
    type Writer = Buffer;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

async fn capture(filter: &str, method: &str, uri: &str) -> String {
    let buffer = Buffer::default();
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(filter))
        .with_writer(buffer.clone())
        .with_ansi(false)
        .finish();
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .body(Body::empty())
        .unwrap();
    // Attach the subscriber to the future so every span/event the router emits while it runs is
    // captured, regardless of which worker thread polls it.
    app(AppState::default())
        .oneshot(request)
        .with_subscriber(subscriber)
        .await
        .unwrap();
    buffer.contents()
}

// A 5xx logs exactly one ERROR line, and it carries route, request id, cause, classification, and
// latency — no separate "handler failed" event.
#[tokio::test]
async fn failure_logs_one_error_line_with_full_context() {
    let logs = capture("debug", "GET", "/api/tasks").await;

    assert_eq!(
        logs.matches("request failed").count(),
        1,
        "exactly one failure line expected:\n{logs}"
    );
    assert!(!logs.contains("request handler failed"), "logs:\n{logs}");
    assert!(!logs.contains("request served"), "logs:\n{logs}");
    for field in [
        "route=/api/tasks",
        "request_id=",
        "method=GET",
        "error=no .plan/ store found",
        "failure=Status code: 500",
        "latency_ms=",
    ] {
        assert!(logs.contains(field), "missing {field:?} in:\n{logs}");
    }
}

// The request span is created at ERROR level, so its route/id fields survive even when RUST_LOG is
// turned down to only show failures.
#[tokio::test]
async fn failure_context_survives_a_warn_filter() {
    let logs = capture("warn", "GET", "/api/tasks").await;

    assert!(logs.contains("request failed"), "logs:\n{logs}");
    assert!(!logs.contains("request served"), "logs:\n{logs}");
    for field in [
        "route=/api/tasks",
        "request_id=",
        "error=no .plan/ store found",
    ] {
        assert!(logs.contains(field), "missing {field:?} in:\n{logs}");
    }
}

#[tokio::test]
async fn debug_logs_one_line_per_served_request() {
    let logs = capture("debug", "GET", "/health").await;

    assert_eq!(
        logs.matches("request served").count(),
        1,
        "exactly one per-request line expected:\n{logs}"
    );
    assert!(!logs.contains("request failed"), "logs:\n{logs}");
    for field in [
        "status=200",
        "route=/health",
        "request_id=",
        "method=GET",
        "latency_ms=",
    ] {
        assert!(logs.contains(field), "missing {field:?} in:\n{logs}");
    }
}

#[tokio::test]
async fn info_filter_stays_quiet_for_a_fast_success() {
    let logs = capture("info", "GET", "/health").await;
    assert!(
        logs.trim().is_empty(),
        "expected no per-request logs:\n{logs}"
    );
}
