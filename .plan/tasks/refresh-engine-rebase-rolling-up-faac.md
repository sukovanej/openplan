---
status: todo
deps:
- daemon-ambient-writer-accumulate-e2f2
---
# Refresh engine: rebase rolling-updates onto main (event-driven)

**Phase 4** of the rolling-updates plan
([[design-a-continuous-changes-accu-2380]] §7.11). Keep
`refs/open-plan/rolling-updates` reconciled onto `main` so it is always
`main` + the pending ambient stack, worktree-less, conflict-gated.

## Coordination constraint

The AmbientWriter ([[daemon-ambient-writer-accumulate-e2f2]]) is the **sole
writer** of the ref. Refresh also moves the ref, so it must be a **command that
same actor processes**, never a second writer. Phase 4 = a scheduler that
enqueues `Refresh`, plus the `Refresh` handler; it reuses Phase 1's
`replay_onto` ([[ref-plumbing-in-op-git-for-the-r-0489]]).

## 1. A `main`-moved signal (op-watch / op-api)

`op-watch::Watcher` already watches `.git` refs/HEAD and emits debounced
`ChangeEvent`s. Add `ChangeEvent::BranchTipChanged { branch, commit }` so the
scheduler gets a clean trigger: a code-only `main` commit changes no task yet
must still trigger a refresh, so inferring from per-task diffs is insufficient.

## 2. `RefreshScheduler`

- Subscribes to `BranchTipChanged` for `repo.default_branch()`.
- **Debounces on a ~1 min quiet window** (each move resets the timer) -> enqueues
  one `Refresh`.
- **Periodic sweep** (timer) and **startup reconcile** (on boot) enqueue the same
  idempotent `Refresh` — safety nets for watcher events dropped under load or
  missed while the daemon was off. NOT a daemon-down path.
- If the ref is already based on `main`, the handler no-ops.

## 3. `Refresh` handler (inside the ref-owner actor)

Flush staged ambient edits first, then:
- `divergence = merge_bases_against(main_tip, [rolling_tip])`; `pending` = the
  ambient commits on `rolling` since `divergence` (this only enumerates the
  commit range — it is **not** the 3-way merge base).
- `replay_onto(main_tip, pending)` — worktree-less; each commit is replayed with
  its own parent as the per-step base (Phase 1), so the section driver fires per
  step.
- `Done { tip }` -> CAS `rolling` old->new; `SyncState` -> `InSync` / `Pending`.
- `Blocked { last_good, tasks }` -> **hold** the ref at its last good tip, set
  `SyncState::Blocked`, record `tasks` as **conflicted** (machine-local per §7.7,
  in `SyncStatus.conflicted`, surfaced to the UI). Retry on the next refresh once
  a human reconciles. Show `Syncing` while the replay runs.

## Scope

Scheduler + `Refresh` handler + the conflicted-task set. Publish (Phase 5) stays
separate and manual; refresh never touches `main`. UI rendering of
`Blocked` / conflicted is Phase 7 — Phase 4 only populates the state.

## Verify

Integration tests (`crates/op-server/tests/`):
- moving `main` (task-changing and code-only commits) triggers exactly one
  refresh after the quiet window; a burst coalesces to one.
- clean refresh: non-overlapping edits -> ref rebased onto new `main`, tip
  advances, `SyncState` clears.
- conflict: an injected same-section divergence -> ref held at last good tip,
  `SyncState::Blocked`, the right task ids in `conflicted`; next refresh retries.
- startup reconcile: a `main` move applied while the "daemon" was down is picked
  up on boot.
