---
status: backlog
created: 2026-08-02T13:56:27Z
parent: ./00079-multi-repo-daemon-one-daemon-and.md
---
# Project container, registry.toml, and project-prefixed routes

Introduce the shape without a change in behavior. Move everything repo-scoped
in `AppState` into a `Project`. Make the daemon load a registry. Seed the
registry with one entry from `--root` when the file does not exist. Add the
`/api/projects/{name}` prefix to the task routes. With one registered
project, the daemon serves exactly what it serves today.

## `Project` and `AppState`

`Project` owns what `AppState` (`op-server/src/lib.rs`) holds for its one
repository today: `name`, `root`, `repo`, `store`,
`index: Arc<Mutex<Index>>`, `ids: Arc<IdCounter>`, and the presence
`Registry`. `AppState` keeps only the daemon-wide parts:
`projects: RwLock<BTreeMap<String, Arc<Project>>>`, `shutdown`, `health`, and
`events`.

Locks: the map's `RwLock` guards membership only. Resolve the
`Arc<Project>`, then release the read guard before you touch the index. Each
project keeps its own index `Mutex`. A rebuild in one project (object-DB
reads, flock waits) then never blocks reads in an other project. `IdCounter`
stays per project. This keeps the rule "a number is issued at most once per
repository".

## Registry

Put `~/.plan/registry.toml` adjacent to `daemon.json` in `Home::dir()`
(`op-cli/src/daemon.rs`). Put the read/write code in `op-server`. Give the
type a name that does not collide with `op_presence::Registry`. `serve.rs`
passes the path in. At start, `serve.rs` loads the file and builds one
`Project` for each entry. When no file exists, `serve.rs` writes a one-entry
registry from `--root`. This is the first-run migration. A registered path
whose store or repo does not open must not stop the daemon. Until the
isolation work lands, write a log line and skip the entry.

## Routes

Move `/api/tasks`, `/api/board`, `/api/tasks/{id}`, and `/api/config` under
`/api/projects/{name}/…`. A handler resolves the project from the path
segment. When the name is unknown, answer 404 and name the known projects.
`/health`, `/api/events`, and `/admin/shutdown` stay daemon-wide.

Keep the unprefixed routes for now. They delegate to the one registered
project, because the embedded SPA calls them. The SPA child deletes them when
they have no callers. `/api/config` follows the same rule: its replacement is
the project entry, which carries the abbreviation.

`DaemonInfo.repo` keeps its meaning for now; fill it from the one project.
The CLI child replaces the field together with its consumers
(`ensure_same_repo`, `task_updated`).

The watcher wiring in `serve.rs` stays single. It reads the repo and the
store through the one project entry. One watcher per project is work for the
next child.

## Acceptance

- With one registered project, each task route gives byte-identical answers
  under both spellings. The existing `op-server` tests pass against the
  prefixed routes.
- A start with `--root` and no registry file writes the one-entry registry. A
  restart with the file present does not write the file again.
- The OpenAPI spec builds. `mise run generate-web-client` waits for the SPA
  child.
- `cargo build && cargo test && cargo fmt --check && cargo clippy -- -D warnings` pass.
