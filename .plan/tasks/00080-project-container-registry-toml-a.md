---
status: backlog
created: 2026-08-02T13:56:27Z
parent: ./00079-multi-repo-daemon-one-daemon-and.md
---
# Project container, registry.toml, and project-prefixed routes

Introduce the shape without changing behaviour: everything repo-scoped in
`AppState` moves into a `Project`, the daemon loads a registry (seeding it with
one entry from `--root` when the file does not exist yet), and task routes gain
a `/api/projects/{name}` prefix. With one registered project the daemon serves
exactly what it serves today.

## `Project` and `AppState`

`Project` owns what `AppState` (`op-server/src/lib.rs`) holds per-repo today:
`name`, `root`, `repo`, `store`, `index: Arc<Mutex<Index>>`,
`ids: Arc<IdCounter>`, and the presence `Registry`. `AppState` keeps only the
daemon-wide parts: `projects: RwLock<BTreeMap<String, Arc<Project>>>`,
`shutdown`, `health`, `events`.

Locking: the map's `RwLock` guards membership only — resolve the `Arc<Project>`
and drop the read guard before touching the index. Each project keeps its own
index `Mutex`, so one project's rebuild (object-DB reads, flock waits) never
blocks another's reads. `IdCounter` stays per project, preserving "a number is
issued at most once per repository".

## Registry

`~/.plan/registry.toml` lives beside `daemon.json` in `Home::dir()`
(`op-cli/src/daemon.rs`). The read/write code lives in `op-server` (the type
needs a name that does not collide with `op_presence::Registry`); `serve.rs`
passes the path in. `serve.rs` loads it at start, builds one `Project` per
entry, and — first-run migration — writes a one-entry registry from `--root`
when no file exists. A registered path whose store or repo cannot be opened
must not stop the daemon; until the isolation work lands, log and skip it.

## Routes

`/api/tasks`, `/api/board`, `/api/tasks/{id}`, and `/api/config` move under
`/api/projects/{name}/…`; handlers resolve the project from the path segment
and answer 404 naming the known projects when it misses. `/health`,
`/api/events`, and `/admin/shutdown` stay daemon-wide.

The unprefixed routes do not disappear yet: they delegate to the sole
registered project, because the embedded SPA still calls them. The SPA child
deletes them once nothing does. `/api/config` follows the same rule: its
replacement is the project entry itself, which carries the abbreviation.

`DaemonInfo.repo` keeps its meaning for now (populated from the sole project);
the CLI child replaces it together with its consumers (`ensure_same_repo`,
`task_updated`).

The watcher wiring in `serve.rs` stays single — it reads repo and store through
the one project entry; one watcher per project is the next child's work.

## Acceptance

- With one registered project, every task route answers byte-identically under
  both spellings, and `op-server`'s existing tests pass against the prefixed
  ones.
- Starting with `--root` and no registry file writes the one-entry registry;
  restarting with the file present does not rewrite it.
- OpenAPI spec regenerates cleanly (`mise run generate-web-client` is deferred
  to the SPA child; here the spec just must build).
- `cargo build && cargo test && cargo fmt --check && cargo clippy -- -D warnings` pass.
