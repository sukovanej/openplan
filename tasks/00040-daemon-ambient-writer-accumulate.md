---
status: todo
created: 2026-07-26T15:40:55Z
parent: ./00039-continuous-changes-accumulation-v.md
dependencies:
- ./00043-ref-plumbing-in-op-git-for-the-r.md
---
# Daemon ambient writer: accumulate + serialize into rolling-updates

**Phase 2** of the rolling-updates plan
([[./00023-design-a-continuous-changes-accu.md]] §7.11). The daemon becomes the
**sole serialized writer** of `refs/open-plan/rolling-updates`, accumulating
ambient edits worktree-less via the Phase 1 primitives
([[./00043-ref-plumbing-in-op-git-for-the-r.md]]).

## Why a new writer

Every write today needs a live worktree: `op-server`'s `patch_task` /
`delete_task` / `create_task` resolve a target through
`write_branch` -> `index.live_store(&branch)`, which yields a `Store` only for a
branch **checked out in some worktree** (else `NotWritable`). `rolling-updates`
is not a branch and has no worktree, so it cannot go through `Store`. Phase 2 is
a second, parallel writer built on `op-git::{commit_overlay, update_ref}`.

## `AmbientWriter` actor

A single tokio task owning the ref (the "sole serialized writer, no
cross-process lock" of the SPEC), behind an mpsc command channel:

- **State:** current `tip` (rolling commit) + `staged: HashMap<TaskId, Upsert(Task)|Delete>`
  (the coalescing buffer).
- **On command** (`Create(Task)` / `Patch(id, TaskPatch)` / `Delete(id)`, sent by
  Phase 3's router): base = `staged[id]` if present, else the committed tip's
  blob for `id` (via `op-git`), else empty for create; apply; update `staged`;
  **ack the caller immediately** with the resulting `TaskDetail` (HTTP handler
  stays synchronous as today). Arm/extend a **debounce timer**.
- **On debounce fire** (quiet window, or max-batch / max-size cap): flush
  `staged` into one `commit_overlay(tip, changes, msg)` ->
  `update_ref(ROLLING_REF, new, expected=tip)` CAS -> advance `tip`, clear
  `staged`, emit `ChangeEvent`, update sync-status. Rapid keystrokes / drag
  collapse into few commits.

**Read consistency:** reads (`list_tasks` aggregation) otherwise see only the
committed tip, so an un-flushed edit would briefly vanish. The actor exposes a
`snapshot()` of `staged` that the index overlays for the ambient lane. Required,
not optional.

## Sync status (for the API / Phase 7 UI)

Actor maintains a `watch::Sender<SyncStatus>` in `AppState`:

```rust
enum SyncState { InSync, Pending(u32), Syncing, Blocked, Offline }
struct TaskChange { id: String, title: String, kind: ChangeKind }
struct SyncStatus { state: SyncState, pending: Vec<TaskChange> }
```

`pending` = diff(main tree, rolling tip tree) ∪ `staged`. Exposed via a new
`GET /api/sync` and pushed over the existing SSE channel.

## Scope

Writer + status plumbing and its **send API** only. Does **not** decide which
writes are ambient (Phase 3 routing), run refresh/publish (Phase 4/5), or render
UI (Phase 7). Blocked/Syncing states are wired here but only driven once those
phases land.

## Verify

Integration tests in `crates/op-server/tests/`, driving the ambient command
channel directly (no routing):
- N rapid edits within the debounce window -> one commit; `tip` advances once.
- interleaved edits to the same id coalesce to the final state (last wins).
- a create then delete of the same id in one window nets to no task.
- `snapshot()` overlay: a task edited but not yet flushed reads back through the
  aggregation.
- `GET /api/sync` reports `Pending(n)` with the right per-task changes; returns
  `InSync` when rolling tip == main.
