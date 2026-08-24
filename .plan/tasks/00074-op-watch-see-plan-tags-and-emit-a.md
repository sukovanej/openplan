---
status: in_review
created: 2026-07-31T14:08:22Z
parent: ./00012-tags-registered-labels-name-colo.md
dependencies:
- ./00073-tags-wire-surface-op-api-dtos-ap.md
---
# op-watch: see .plan/tags and emit a coarse Change::Tags

Phase 4 of [[./00012-tags-registered-labels-name-colo.md]]: make the watcher see the tag registry, so CLI and hand edits reach `/api/events`.

Today `.plan/tags/` is invisible three times over: worktree watches cover only `.plan/tasks` (`crates/op-watch/src/lib.rs:389-394`), the served store's `.plan` is watched non-recursively (`:385-388`), and `is_relevant` requires a `tasks`/`config` path (`:414-438`).

## Scope
- `watch_paths`: add each worktree's `.plan/tags` (recursive).
- `is_relevant`: accept paths with a `tags` component under `.plan`.
- Snapshot (`crates/op-watch/src/lib.rs:219`): carry the tag files' blob oids per live worktree alongside tasks; `emit_diff` (`:312`) emits one coarse `Change::Tags` when they differ — no per-tag diffing.
- Bridge in `op-cli/src/serve.rs` (`:120-131`): `Change::Tags` → `ChangeEvent::TagsChanged` on the broadcast.

## Verify
`crates/op-watch/tests/watch.rs`: a write to a worktree's `.plan/tags/` yields `Change::Tags`; a task-only edit does not; `.git`-side churn still doesn't. `op-cli` daemon test: a CLI-side tag write while the daemon runs surfaces `tags_changed` on `/api/events`.
