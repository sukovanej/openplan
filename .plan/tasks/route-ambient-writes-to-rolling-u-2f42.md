---
status: todo
deps:
- daemon-ambient-writer-accumulate-e2f2
---
# Route ambient writes to rolling-updates (server + CLI)

**Phase 3** of the rolling-updates plan
([[design-a-continuous-changes-accu-2380]] §7.11). Apply the routing table at
the write boundary so ambient/triage edits reach the daemon's AmbientWriter
([[daemon-ambient-writer-accumulate-e2f2]]) instead of a feature branch.

## Two write paths to touch

- **CLI** (`create` / `set` / `delete` in `op-cli/src/main.rs`) writes directly
  via `Store::discover(root)` — daemon-independent, the §7.1 writes-local path.
- **Server** (`patch_task` / `create_task` / `delete_task`) resolves a target via
  `write_branch` -> `index.live_store(&branch)`.

Neither may touch `rolling-updates` directly: the daemon is the **sole
serialized writer** (Phase 2). Routing "ambient" means handing the edit to the
AmbientWriter command channel — and for the CLI that means going **through the
daemon over HTTP**. Consequence to state plainly: an ambient CLI edit now
requires a running daemon; feature-branch writes stay daemon-independent.

## Server — `WriteTarget` at the boundary

```rust
enum WriteTarget { Branch(String), Ambient }
```

Resolve per request: explicit `?target=ambient` -> `Ambient`; `?branch=X` ->
`Branch(X)`; neither -> default by the serve-root worktree's branch: if it is
`repo.default_branch()` (trunk) -> `Ambient`, else `Branch(current)`. The three
write handlers then split:
- `Branch(b)` -> today's `live_store` path, unchanged.
- `Ambient` -> send `Create` / `Patch` / `Delete` to the Phase 2 channel, await
  ack, return the same `TaskDetail`.

Encodes "UI global view -> ambient" (client sends `target=ambient`) and "UI
swimlane scoped to a branch -> that branch" (`?branch=`).

## CLI — `--ambient` + trunk auto-detect

- A write is ambient when `--ambient` is passed **or** the current worktree's
  branch equals `repo.default_branch()` (trunk). This implements "CLI/agent in
  the trunk worktree -> rolling-updates" and enforces CLAUDE.md's "never write to
  the main checkout" by rerouting rather than refusing.
- Ambient path: forward the write to the daemon's task route with
  `target=ambient` via new `op-client` write methods; **error clearly if the
  daemon is down** (the sole-writer invariant forbids a local file-lock
  shortcut).
- Feature-worktree path: unchanged local `Store` write.

## Scope

Target resolution + dispatch only. AmbientWriter (Phase 2) does the
accumulation; refresh/publish (Phase 4/5) and UI (Phase 7) untouched.
`op-client` gains write methods (today only `health` / `shutdown`).

## Verify

- Server: `?target=ambient` create/patch/delete lands on the AmbientWriter (ref
  advances), not any branch; `?branch=X` still writes branch X; no-param write on
  a trunk serve-root routes ambient, on a feature serve-root routes to that
  branch.
- CLI (`crates/op-cli/tests/`): `set` from a trunk worktree with a live daemon
  hits the ambient endpoint; `--ambient` from a feature worktree does too; a
  feature-worktree write without `--ambient` stays local; ambient write with the
  daemon down errors clearly.
