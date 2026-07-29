---
status: todo
created: 2026-07-26T15:40:55Z
parent: '39'
deps:
- '44'
---
# Publish: fast-forward main to the rolling-updates tip (API + CLI)

**Phase 5** of the rolling-updates plan
([[23]] §7.11). Manual, explicit,
**fast-forward-only** advance of `main` to the `rolling-updates` tip. Never a
merge, never a force; publish cannot conflict (Phase 4 forces conflicts to
surface at refresh).

## Wrinkle: `main` may be checked out

A bare ref CAS on `main` would desync a worktree that has `main` checked out
(torn index / working tree, §7.8). Handle both cases.

## Design

**Publish is a command on the ref-owner actor**
([[40]]). It advances `main` (not `rolling`)
but reads the rolling tip and must not race the refresh loop
([[44]]); serializing it through the actor
that already owns ref ops avoids the interleave. Manual only, never timed.

**Fast-forward, two cases** (uses Phase 1
[[43]] `is_fast_forward` / `update_ref`):
- `main` **not checked out** -> worktree-less: verify `is_fast_forward(main,
  rolling)`, then CAS `main` old->rolling.
- `main` **checked out in W** -> guarded `--ff-only` that also updates W's index +
  working tree (gix, or `git merge --ff-only` in W); **refuse** if W's `.plan/` is
  dirty or the daemon has W busy (§7.8). The delta is only `.plan/` task files
  (ambient edits were routed away from W in Phase 3), so code in W is untouched.

**Only failure mode — non-FF.** `main` can move after the last refresh, making
rolling no longer a fast-forward; the CAS on `main` catches the race. Refuse with
a **retriable** error ("main moved — refresh, then publish"); the Phase 4
main-move watcher refreshes automatically, then publish succeeds.

## Surface

- `POST /api/publish` -> FFs `main` to the rolling tip; returns the new `main`
  commit + count published. `409` non-FF (refresh needed); `423`/`409` if W is
  busy/dirty. `SyncState::Syncing` during, `InSync` after.
- CLI `oplan publish` -> calls the endpoint; **daemon-only** (errors if the
  daemon is down), since the daemon owns the ref lifecycle.

## Scope

Publish path + API/CLI only. The review popover carrying the Publish button and
the Blocked/Resolve UI are Phase 7.

## Verify

- clean publish: rolling ahead of `main` -> `main` FFs to rolling tip; pending
  count -> 0; `InSync`.
- worktree-less vs checked-out: publish with `main` unchecked-out advances the
  ref; with `main` checked out in a clean worktree it updates that working tree;
  a dirty `.plan/` in W is refused.
- non-FF: move `main` after refresh so rolling is behind -> `409` retriable, no
  merge commit, `main` unchanged.
- `oplan publish` with the daemon down errors clearly.
