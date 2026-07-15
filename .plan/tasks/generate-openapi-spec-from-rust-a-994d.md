---
status: backlog
---
# Generate OpenAPI spec from Rust API; generated Effect HTTP client package

## Goal

Generate an OpenAPI spec from the `op-server` axum API and use it to generate a
typed Effect HTTP client, published as a new package in the `web` TypeScript
monorepo so `@open-planner/app` consumes a generated client instead of
hand-written `fetch` calls.

## Scope

### Rust: emit the OpenAPI spec

- Describe the `op-server` HTTP surface as OpenAPI 3.1. Current routes:
  `GET /health`, `GET /api/events` (SSE), `GET /api/tasks` +
  `POST /api/tasks`, `GET|PATCH|DELETE /api/tasks/{id}`, `POST /admin/shutdown`.
- Derive schemas from the existing wire types in `op-api` (the DTOs already
  serde-serialized on the wire) rather than duplicating them — annotate those
  types so the spec stays in lockstep with what the server actually returns.
- Use **`utoipa` + `utoipa-axum`** (https://github.com/juhaku/utoipa,
  https://docs.rs/utoipa-axum). Chosen over `aide` because it emits OpenAPI
  3.1, `#[derive(ToSchema)]` honors serde attributes on the `op-api` DTOs, and
  `utoipa-axum::OpenApiRouter` binds each route to its `#[utoipa::path]` so
  routes can't drift from the spec. Cost: a per-handler `#[utoipa::path]`
  attribute (~6 routes, acceptable).
- Expose the spec via `oplan server openapi`, which prints the OpenAPI 3.1 JSON
  to stdout (no daemon needed). Treat the spec as a build intermediate, not a
  committed artifact: the `generate-web-client` mise task runs
  `oplan server openapi` into a git-ignored `openapi.json` inside the package,
  then feeds it to the client generator. The committed source of truth is the
  Rust API; the checked-in output is the generated `src/index.ts`.
- Return API errors as a JSON `ApiErrorBody { message }` (not `text/plain`) and
  document them with `body = ApiErrorBody` on the 4xx responses. This gives the
  generated client typed, tagged errors *and* keeps the generated file free of
  unused-helper lint/`noUnusedLocals` errors — so it is fully formatted and
  linted with no per-file skips.

### Web: generated Effect HTTP client package

- New workspace package `@open-planner/api-client` under `web/packages/`.
- Generate the client from `openapi.json` with **`@effect/openapi-generator`**
  — the spec-first codegen in the effect monorepo
  (https://github.com/Effect-TS/effect/tree/main/packages/tools/openapi-generator).
  On npm at `4.0.0-beta.98`, matching our `effect` catalog pin; peer-deps
  `effect` + `@effect/platform-node`. Its `openapigen` bin reads a spec and
  writes generated source to stdout (redirected straight to the package entry):

  ```sh
  openapigen -s openapi.json -f httpclient -n TasksClient > src/index.ts
  ```

  Flags: `-s/--spec`, `-n/--name` (default `Client`), `-f/--format`
  (`httpclient` | `httpclient-type-only` | `httpapi`, default `httpclient`),
  `-p/--patch` (JSON patch(es) applied to the spec before generation). Use
  `httpclient` for a typed Effect client; `httpapi` if we'd rather emit an
  `HttpApi` module and derive the client via `HttpApiClient.make`.
- Wire generation into a package script (`pnpm generate`) so regenerating from
  an updated spec is one command.
- Add `@effect/openapi-generator` + `@effect/platform-node` to the catalog, the
  package to `pnpm-workspace.yaml`, and it to the monorepo `build` / `typecheck`
  graph.

### App: consume the client

- Replace the hand-written task `fetch` calls in `@open-planner/app` with the
  generated `@open-planner/api-client`. (SSE `/api/events` can stay as-is if the
  generator doesn't cover event streams — call that out.)

## Open questions

- `httpclient` vs `httpapi` output format — decide when we see the generated
  shape against our endpoints.
- SSE `/api/events`: the generator targets request/response operations; expect
  it to skip or mishandle the event stream — keep the app's `EventSource`
  hand-written and exclude the route (or `-p` patch it out) if it warns.

## Done when

- `op-server` produces an OpenAPI 3.1 spec derived from `op-api` wire types,
  reachable as a committed `openapi.json` and/or a server route/CLI subcommand.
- `web/packages/api-client` (`@open-planner/api-client`) exists, generates a
  typed Effect client from the spec via a `pnpm generate` script, and builds.
- `@open-planner/app` uses the generated client for the task endpoints.
- Rust checks pass (`cargo build`, `cargo test`, `cargo fmt --check`,
  `cargo clippy -- -D warnings`) and web checks pass (lint + typecheck + build).
