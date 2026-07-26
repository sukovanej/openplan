---
status: in_progress
---
# Track created and updated timestamps per task

Give every task a `created` timestamp (stored, always set) and a `updated`
timestamp (derived from git history), and surface both through the API.

## `created` — stored in frontmatter, always set

- New named field on `Frontmatter` (alongside `parent`/`deps`), an RFC3339 UTC
  string. Named, not left in `extra`, so it gets a real type and the write path
  never drops it.
- Written on task creation from the daemon's clock.
- The write path (`set_status`, reparent, etc.) preserves it unchanged.
- Users may hand-edit `created`. When a task diverges across branches, the
  headline cell's value wins (the headline is already the most recently changed
  branch, so "latest revision wins" falls out for free).

### Migration (backfill existing tasks)

Every existing task lacks `created`; backfill so the field is *always* present.

- For each task, read its git first-appearance and use that commit's
  **author-time** as `created`.
- Commit the backfill **one task per commit**, with each commit's author-date
  **backdated to that task's current real last-change time**. This is required:
  `updated` is derived from author-time (below), so a single bulk commit — or
  commits dated "now" — would reset every task's `updated` to migration day. Per-
  task backdated commits keep the derived `updated` truthful.

## `updated` — derived from git, not stored

- Computed by walking history for each task's most recent blob change, using
  **author-time** (stable across rebase/squash/amend), via the existing
  `op_git::task_change_times` low-level walk run over the full task set.
- `task_change_times` currently reads `commit_time()`; switch this path to
  author-time.
- Do **not** reuse the index `recency` field — that is unrelated
  headline-selection machinery (only computed for multi-branch tasks, carries
  `0`/`u64::MAX` sort placeholders, never a real date).
- Any blob change counts as an update — status flip, body typo, reparent — with
  no semantic filtering over *what* changed.
- An uncommitted working-tree edit has no commit to date; render it as "now".

### Known gap

`task_change_times` bails at `HISTORY_WALK_BUDGET` (4096 commits). A task whose
last edit sits deeper than that reads as unknown. Realistically never hit for a
task tracker — note it, don't engineer around it.

## Surface + render

- Add `created` and `updated` to `TaskView` / `TaskDetail` in `op-api`.
- Render clamp: display `updated = max(created, updated)` as a backstop against
  an inverted pair (e.g. a user hand-setting `created` after the last edit).

## Out of scope

- Validation of `created` (not in the future, `created <= updated`): deferred to
  a future `oplan check` command.
- Refactoring the index `recency` sentinels (`0`/`u64::MAX`) into a 3-variant
  ordered enum: a worthwhile cleanup, but orthogonal to this task.

## Refs / reuse

- `crates/op-task/src/lib.rs` — `Frontmatter`, `Task` (add `created`).
- `crates/op-git/src/lib.rs` — `task_change_times` (add author-time variant /
  first-appearance lookup).
- `crates/op-api/src/lib.rs` — `TaskView`, `TaskDetail` (surface both fields).

## Tests (in `tests/`)

- Frontmatter round-trips `created`; write-path mutations preserve it.
- New task gets `created` set from the injected clock (test seam for the clock).
- `updated` derivation returns the author-time of the last blob change; a
  status-only change bumps it; an uncommitted edit reads as "now".
- Migration sets `created` on a task lacking it and leaves `updated` unchanged
  (the backdated-commit invariant).
- Render clamp: a `created` after `updated` displays `updated == created`.
