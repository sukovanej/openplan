---
status: done
---

# Task CRUD across the store, daemon, and CLI

## Goal
Turn the skeleton into a working store: create, read, update, and delete task files,
reachable both from the `oplan` CLI (local, always-on) and from the daemon over HTTP (for the
UI). One write implementation in `op-store`; both faces call it. Files stay the source of
truth — the daemon observes CLI writes through its watcher, it does not own them.

## Design
- **`op-store` owns all mutation.** `create` / `write` / `delete`, each under the existing
  per-file advisory lock, each an atomic temp-write + rename (§6, §7.8) so the watcher never
  reads a torn file. This is the single code path; the CLI and the daemon both delegate here.
- **Reads are local-first for v1.** The daemon's aggregated matrix (`op-index`) is still empty,
  so CLI reads resolve against `op-store` directly. Daemon-aggregated / cross-branch reads land
  with the index task, not here.
- **Writes are local (§7.1).** The CLI writes the current worktree's `.plan/` directly — the
  always-available path that works headless and with the daemon down. The daemon exposes the
  same CRUD over HTTP so the UI can drive it; those handlers call the identical `op-store`
  functions. Routing CLI *writes* through the daemon for busy-worktree gating (§7.8) is
  deferred until that gating exists.
- **Identity = filename (§3.1).** `create` generates `<title-slug>-<rand>.md` (kebab slug of the
  title, truncated; short random suffix), retrying on the rare collision. `id` is never written
  into the file. `title` is the body's single `# H1`, never frontmatter.

## CLI surface (this task)
```
oplan create "<title>" [--parent <id>] [--status <s>] [--dep <id> ...]   # → prints new id
oplan list   [--status <s>] [--parent <id>] [--json]                     # extend existing
oplan get    <id> [--json]                                               # whole file, or metadata as json
oplan show   <id>                                                        # metadata only (status/parent/deps)
oplan set    <id> <field> <value>                                        # validated: status | parent | deps
oplan delete <id> [--yes]                                                # remove the file
```
(Daemon reachability / `server ping` is out of scope here — see [[daemon-lifecycle-45b1]].)
- `--json` on reads emits `op-api` types (agent-facing); default is the human table.
- `set` validates: `status` against the enum, `parent`/`deps` are existing ids; rejects a
  `parent` that would point a task at itself.
- `delete` is a hard file removal; note the soft alternative is `set <id> status cancelled`.

## Daemon / HTTP (this task)
Add a `Store` (resolved from the serve root) to `AppState`, and REST routes delegating to
`op-store`:
```
GET    /api/tasks            # list summaries
POST   /api/tasks            # create → { id }
GET    /api/tasks/:id        # full task (frontmatter + body)
PATCH  /api/tasks/:id        # update metadata (status/parent/deps)
DELETE /api/tasks/:id        # remove
```
- Request/response bodies are new `op-api` DTOs (`CreateTask`, `TaskPatch`, `TaskView`),
  reusing `TaskSummary`.
- Map `StoreError` → status codes (not-found → 404, validation → 400, io → 500).
- The existing watcher already fires on these writes; no index wiring required here — a
  `TaskChanged` log line is enough for now.

## Crate changes
- **op-task**: `Task::new(title, status)` building `# {title}\n` body + default frontmatter;
  helpers to set `status` / `parent` / `deps`. Keep parse↔serialize roundtrip intact.
- **op-store**: `create(&Task) -> Result<String>` (slug+rand id, collision retry, atomic
  write), `write(id, &Task)` (lock + atomic temp+rename), `delete(id)`, `exists(id)`. Add
  `StoreError::NotFound { id }` / a validation variant as needed. Pick a small randomness
  source (`getrandom` or `rand`) and hoist it to `[workspace.dependencies]`.
- **op-api**: add `CreateTask`, `TaskPatch`, `TaskView` (+ reuse `TaskSummary`); derive
  serde.
- **op-server**: `Store` in `AppState`; the five routes above; `StoreError` → HTTP mapping.
- **op-cli**: `create` / `get` / `show` / `set` / `delete` subcommands; a thin `reqwest`
  client is optional for reads and unused for writes in v1. Daemon lifecycle (`server ping`
  replacing the `ping` stub) lands in [[daemon-lifecycle-45b1]].

## Acceptance criteria
- [ ] `oplan create "Wire the parser"` writes `.plan/tasks/<slug>-<rand>.md` with `status: todo`
      and `# Wire the parser`, and prints the id; a second create with the same title gets a
      distinct id.
- [ ] `oplan show <id>` and `oplan get <id> --json` print the task; `get` on a missing id exits
      non-zero with a clear message.
- [ ] `oplan set <id> status in_progress` updates only frontmatter; the body is byte-for-byte
      unchanged. Invalid status / non-existent parent is rejected non-zero.
- [ ] `oplan delete <id> --yes` removes the file; `list` no longer shows it.
- [ ] `oplan list --json --status todo` filters and emits valid `op-api` JSON.
- [ ] Daemon: `POST/GET/PATCH/DELETE /api/tasks[/:id]` round-trip a task; missing id → 404,
      bad body → 400. Covered by `tower::ServiceExt::oneshot` tests (deps already present).
- [ ] `op-store` unit tests: create→read→update→delete roundtrip; concurrent `write` to the
      same id serializes; atomic write leaves no partial file on the happy path.
- [ ] `cargo build`, `cargo test`, `cargo fmt --check`, `cargo clippy -- -D warnings` all pass.

## Out of scope (follow-up tasks)
- Section/block addressing and splice edits (`get/set -t 'Section'`, `append`) — the `op-md`
  target-resolution task.
- Building the task×branch matrix and cross-branch / `--all-branches` reads — the `op-index`
  task.
- Presence / `claim` / `release` and busy-worktree write gating (§7.6, §7.8).
- Section-aware merge driver, and the actual web UI consuming these routes.

## Notes
- Keep writes minimal-diff: `set` mutates frontmatter and re-serializes only the frontmatter
  block; it must not reflow the markdown body (full section-splice fidelity comes with op-md).
- `blocked` stays computed from unmet `deps`, never stored (§3.1) — out of scope to compute
  here; just persist `deps`.
- This task dogfoods itself: once `create` works, new task files should be made with `oplan`.
