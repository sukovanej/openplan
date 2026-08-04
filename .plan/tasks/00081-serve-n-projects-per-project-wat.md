---
status: backlog
created: 2026-08-02T13:56:32Z
parent: ./00079-multi-repo-daemon-one-daemon-and.md
dependencies:
- ./00080-project-container-registry-toml-a.md
---
# Serve N projects: per-project watcher, failure isolation, dirty-gated rebuilds, /api/projects

Make N projects live at once, each failing alone. Today a broken config or a
vanished root stops the daemon for everything; with N projects that would let
any one repository take down the machine's task UI.

## `/api/projects`

- `GET /api/projects` — one entry per project: `name`, `root`, the git common
  dir, `abbreviation`, and `status` (`ok`, or `error` with a reason string).
  The common dir is what lets a CLI match "my repo" against the list without
  path-prefix guessing.
- `POST /api/projects {path}` — resolve the path to its serve root (main
  checkout with a `.plan` store, as `serve_root` does), slug the directory name
  and uniquify it, write the registry, spin the project up, answer the entry.
  Idempotent: a path whose git common dir is already registered answers the
  existing entry with 200 — auto-registration from the CLI must tolerate two
  racing first writes. No store or no repo is a 400 naming what is missing.
- `DELETE /api/projects/{name}` — drop the entry and tear the project down;
  files on disk are untouched.
- `PATCH /api/projects/{name} {name}` — rename; the registry updates, routes
  answer under the new name.

The daemon writes `registry.toml` only from these handlers.

## Per-project watcher

One `Watcher::start(repo, store, tx)` per project — `op-watch` is already
parameterised by (repo, store) — with the bridge (`serve.rs`) tagging each
`Change` with its project before publishing. A watcher that fails to start
stays what it is today, a logged degradation, but it must also pin that
project's dirty flag (below). Registering tears up a watcher, removing tears
it down.

## Failure isolation

- `reload_config` (`op-cli/src/serve.rs`) demotes the project to
  `error` instead of calling `state.stop()`; the next valid `Change::Config`
  re-promotes it. Routes under a demoted project answer 503 carrying the
  reason.
- `root_removed` becomes a per-project watchdog: two consecutive misses demote
  the project instead of shutting the daemon down. The root reappearing
  (network mount, restored checkout) re-promotes.
- The daemon exits only on signals and `/admin/shutdown`. Zero registered
  projects is a served state, not an error — the UI shows the empty state.

## Dirty-gated rebuild

Every list/board/detail read calls `Index::rebuild`, which walks every local
branch; the merged board multiplies that by N projects per request. Each
project gets a dirty flag: set by its watcher bridge on any `Change` (and on
registration), cleared under the index mutex after a successful rebuild;
`rebuild` is skipped while clean.

Two escapes keep it honest:

- A project without a live watcher is permanently dirty — today's behaviour.
- [[./00052-watcher-misses-working-tree-edit.md]]: a missed fs event currently
  means "stale until refresh"; gated rebuilds would stretch that to "stale
  until the next watched change". A rebuild older than a few seconds counts as
  dirty, bounding the staleness window instead of trusting the watcher fully.

Writes keep their existing rebuild-inside-`write_target` path unconditionally —
the gate is for reads.

## Events

`TaskChanged`, `RefMoved`, and `PresenceChanged` gain a `project` field.
`ConfigChanged` becomes `ProjectsChanged`, covering membership, rename, status,
and abbreviation changes alike — the client's response is the same: re-read
`/api/projects`. The lagged-client nudge stops overloading
`RefMoved { branch: "" }` and becomes an explicit `Resync` variant, since a
fabricated event now needs a fabricated project too.

## Acceptance

- Tests run a daemon over two temp repositories: writes and reads to both
  interleave; ids allocate independently (same number can exist in both).
- Breaking one project's `config.toml` flips it to `error` and its routes
  to 503 while the other keeps answering; restoring the file re-promotes.
- Deleting one project's root demotes it; the daemon keeps serving.
- A clean project's reads skip the rebuild (assert via the index's rebuild
  observability, however cheap — a counter is fine).
- `cargo build && cargo test && cargo fmt --check && cargo clippy -- -D warnings` pass.
