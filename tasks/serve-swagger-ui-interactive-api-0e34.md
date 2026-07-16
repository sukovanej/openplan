---
status: done
---
# Serve Swagger UI (interactive API docs) from op-server

Serve interactive API documentation from the `op-server` daemon so the
OpenAPI 3.1 spec can be browsed and exercised in a browser, not only dumped
via `oplan server openapi`.

## Context

`op-server` already builds a complete OpenAPI spec at compile time from
`utoipa` derive macros (`ApiDoc` / `#[utoipa::path]` handlers in
`crates/op-server/src/lib.rs`; DTOs derive `utoipa::ToSchema` in `op-api`).
Stack: axum 0.8, `utoipa = "5"`, `utoipa-axum = "0.2"`.

The spec is currently only reachable through the CLI. In `app()` the spec half
of `split_for_parts()` is discarded:

```rust
let (router, _) = documented().split_for_parts();
```

There is no HTTP route serving `openapi.json` and no docs UI.

## Library

Use **`utoipa-swagger-ui`** with its `axum` feature — the official utoipa
companion crate, so it drops onto the existing `utoipa = "5"` setup with no
framework churn. Pick the version line that matches `utoipa 5`.

It bundles the Swagger UI dist at build time (adds a `zip` build-dependency);
that build cost is accepted in exchange for the full interactive
("Try it out") UI.

## Suggested approach

1. Add `utoipa-swagger-ui` (feature `axum`) to `op-server`'s `Cargo.toml`.
2. Keep the spec half instead of dropping it in `app()`:
   `let (router, api) = documented().split_for_parts();`
3. Merge the UI into the router before the SPA `static_handler` fallback (so the
   docs path is matched ahead of the catch-all):
   `SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", api)`
4. Ensure the docs path is reachable at a stable URL and does not shadow SPA
   routes.

## Acceptance

- Daemon serves the interactive Swagger UI and the raw `openapi.json` over HTTP.
- The served spec is the same one `oplan server openapi` prints.
- `cargo build`, `cargo clippy -- -D warnings`, and `cargo fmt --check` pass.
