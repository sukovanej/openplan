---
status: in_review
created: 2026-08-02T13:56:32Z
parent: ./00079-multi-repo-daemon-one-daemon-and.md
dependencies:
- ./00080-project-container-registry-toml-a.md
---
# Serve N projects: per-project watcher, failure isolation, dirty-gated rebuilds, /api/projects

Make N projects live at the same time. Make each project fail alone. Today, a
broken config or a deleted root stops the daemon for everything. With N
projects, one broken repository could then remove the task UI for the full
machine.

## `/api/projects`

- `GET /api/projects` — one entry per project: `name`, `root`, the git common
  directory, `abbreviation`, and `status` (`ok`, or `error` with a reason
  string). With the common directory, a CLI can match its own repository
  against the list. It does not have to guess from path prefixes.
- `POST /api/projects {path}` — resolve the path to its serve root (the main
  checkout with a `.plan` store, as `serve_root` does). Make a slug from the
  directory name and make it unique. Write the registry. Start the project.
  Answer with the entry. The route is idempotent: when the path's git common
  directory is already registered, answer the existing entry with 200.
  Auto-registration from the CLI must accept two first writes that race. When
  the path has no store or no repo, answer 400 and name the missing part.
- `DELETE /api/projects/{name}` — remove the entry and stop the project. Do
  not touch the files on disk.
- `PATCH /api/projects/{name} {name}` — rename the project. The registry
  updates. The routes answer under the new name.

Only these handlers write `registry.toml`.

## One watcher per project

Start one `Watcher::start(repo, store, tx)` for each project. `op-watch`
already takes (repo, store) parameters. The bridge (`serve.rs`) adds the
project to each `Change` before it publishes. A watcher that does not start
stays a logged degradation, as today. But it must also pin that project's
dirty flag (see below). Registration starts a watcher. Removal stops it.

## Failure isolation

- `reload_config` (`op-cli/src/serve.rs`) demotes the project to `error`. It
  does not call `state.stop()`. The next valid `Change::Config` promotes the
  project again. Routes under a demoted project answer 503 with the reason.
- `root_removed` becomes a per-project watchdog. Two misses in sequence
  demote the project. They do not stop the daemon. When the root comes back
  (a network mount, a restored checkout), the watchdog promotes the project
  again.
- The daemon stops only on signals and `/admin/shutdown`. Zero registered
  projects is a served state, not an error. The UI shows the empty state.

`serve.rs` refuses to start today when `--root` has no store or no repo, and
when the registry entry for `--root` does not open. Delete both refusals with
this change. A daemon that must serve zero projects cannot also demand one.

## Dirty-gated rebuild

Each list, board, or detail read calls `Index::rebuild`. The rebuild walks
each local branch. The merged board multiplies that cost by N projects for
each request. Give each project a dirty flag. The watcher bridge sets the
flag on each `Change`, and registration sets it. A successful rebuild clears
the flag under the index mutex. While the flag is clear, skip the rebuild.

Two escapes keep the gate safe:

- A project without a live watcher is always dirty. That is today's behavior.
- [[./00052-watcher-misses-working-tree-edit.md]]: a lost fs event today
  means "stale until refresh". With the gate, it would mean "stale until the
  next watched change". Count a rebuild that is more than a few seconds old
  as dirty. This bounds the stale window. Do not put full trust in the
  watcher.

Writes keep their rebuild inside `write_target` in all conditions. The gate
applies to reads only.

## Events

Add a `project` field to `TaskChanged`, `RefMoved`, and `PresenceChanged`.
Replace `ConfigChanged` with `ProjectsChanged`. The new event covers
membership, rename, status, and abbreviation changes. The client response is
the same for all of them: read `/api/projects` again. Stop the overload of
`RefMoved { branch: "" }` as the lagged-client signal. Make it an explicit
`Resync` variant, because the fabricated event would now need a fabricated
project.

## Acceptance

- Tests run one daemon over two temporary repositories. Writes and reads to
  both interleave. The ids allocate independently: the same number can exist
  in both.
- When one project's `config.toml` breaks, its status becomes `error` and its
  routes answer 503. The other project continues to answer. When the file is
  restored, the project is promoted again.
- When one project's root is deleted, the project is demoted. The daemon
  continues to serve.
- Reads on a clean project skip the rebuild. Assert this with cheap
  observability on the index; a counter is sufficient.
- `cargo build && cargo test && cargo fmt --check && cargo clippy -- -D warnings` pass.
