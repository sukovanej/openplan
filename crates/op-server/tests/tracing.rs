mod common;

use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::Request;
use common::*;
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

const TAGS: &str = "/api/projects/test/tags";

// With `broken` true the tasks reference names a commit the repository does not hold, so every read of
// the head fails in storage and the tag list answers 500.
fn state(broken: bool) -> (tempfile::TempDir, AppState) {
    let (dir, state) = git_state();
    if broken {
        std::fs::write(
            dir.path().join(".git/refs/openplan/tasks"),
            "0123456789012345678901234567890123456789\n",
        )
        .unwrap();
    }
    (dir, state)
}

async fn capture(filter: &str, uri: &str, broken: bool) -> String {
    let (_dir, app_state) = state(broken);
    let buffer = Buffer::default();
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(filter))
        .with_writer(buffer.clone())
        .with_ansi(false)
        .finish();
    let request = Request::builder().uri(uri).body(Body::empty()).unwrap();
    // The subscriber rides on the future, so every span and event the router emits is captured,
    // whichever worker thread polls it.
    let response = app(app_state)
        .oneshot(request)
        .with_subscriber(subscriber)
        .await
        .unwrap();
    assert_eq!(response.status().is_server_error(), broken);
    buffer.contents()
}

// A 5xx logs exactly one ERROR line, and it carries the route, the request id, the cause, the
// classification, and the latency.
#[tokio::test]
async fn failure_logs_one_error_line_with_full_context() {
    let logs = capture("debug", TAGS, true).await;

    assert_eq!(
        logs.matches("request failed").count(),
        1,
        "exactly one failure line expected:\n{logs}"
    );
    assert!(!logs.contains("request served"), "logs:\n{logs}");
    for field in [
        "route=/api/projects/{project}/tags",
        "request_id=",
        "method=GET",
        "error=",
        "failure=Status code: 500",
        "latency_ms=",
    ] {
        assert!(logs.contains(field), "missing {field:?} in:\n{logs}");
    }
}

// The request span is made at ERROR level, so its route and id survive a filter that shows only
// failures.
#[tokio::test]
async fn failure_context_survives_a_warn_filter() {
    let logs = capture("warn", TAGS, true).await;

    assert!(logs.contains("request failed"), "logs:\n{logs}");
    assert!(!logs.contains("request served"), "logs:\n{logs}");
    for field in [
        "route=/api/projects/{project}/tags",
        "request_id=",
        "error=",
    ] {
        assert!(logs.contains(field), "missing {field:?} in:\n{logs}");
    }
}

#[tokio::test]
async fn debug_logs_one_line_per_served_request() {
    let logs = capture("debug", "/health", false).await;

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
    let logs = capture("info", "/health", false).await;
    assert!(
        logs.trim().is_empty(),
        "expected no per-request logs:\n{logs}"
    );
}

#[tokio::test]
async fn a_refusal_is_not_logged_as_a_failure() {
    let logs = capture("debug", "/api/projects/test/tasks/OPP-9", false).await;
    assert!(!logs.contains("request failed"), "logs:\n{logs}");
    assert!(logs.contains("status=404"), "logs:\n{logs}");
}
