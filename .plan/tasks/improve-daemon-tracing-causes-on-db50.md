---
status: done
---
# Improve daemon tracing: causes on failed responses, unified tracing, per-request debug logs

Daemon tracing/logging is currently too sparse and inconsistent to debug production issues. Three concrete gaps observed:

## 1. Failed responses give no cause

`tower_http`'s `TraceLayer::new_for_http()` (`crates/op-server/src/lib.rs:97`) logs
failures generically:

```
ERROR tower_http::trace::on_failure: response failed classification=Status code: 500 Internal Server Error latency=15 ms
```

This says nothing about *which* route, *what* request, or *why* it failed. Handlers
that return 500 don't log the underlying error.

Proposed:
- Configure `TraceLayer` with explicit `.make_span_with` (method, path, matched
  route, a request id) and `.on_failure` closures so the failure line carries context.
- Ensure the error type returned by handlers logs the underlying `anyhow`/error cause
  at ERROR before it is flattened into a status code (the `IntoResponse` for the app
  error should `tracing::error!` the source).

## 2. Startup errors bypass tracing

The lock-held path (`crates/op-cli/src/serve.rs:26`) uses `bail!`, so it surfaces as
CLI `error: ...` text with a different format than the tracing-formatted lines:

```
error: another oplan daemon already holds /Users/milansuk/.plan/daemon.lock
```

Decide the intended channel: startup/lifecycle failures that happen after the
subscriber is initialized should go through `tracing::error!` for consistent
formatting; anything before subscriber init is unavoidably plain stderr. Initialize
the subscriber first, then emit lifecycle failures via tracing.

## 3. Missing per-request debug logs

There is no per-request DEBUG log to inspect latency and request flow. Add, gated at
DEBUG level (so INFO stays quiet):
- one line per request: method, path, status, latency, request id
  (`TraceLayer.on_response`);
- request id propagation (`tower_http::request_id`) so a request can be correlated
  across span events;
- optionally slow-request WARN threshold.

## Suggested scope
- Standardize on `tracing` for all daemon lifecycle + request logging.
- Document the `RUST_LOG` / `EnvFilter` levels that surface each tier
  (INFO = lifecycle, DEBUG = per-request).
- Keep the default (no `RUST_LOG`) at INFO so normal runs stay quiet.

Acceptance: a failing request logs route + cause + latency at ERROR; `RUST_LOG=debug`
yields one structured line per request with latency; daemon lifecycle failures use
tracing formatting.
