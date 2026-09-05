---
status: backlog
created: 2026-07-29T15:26:59Z
tags:
- bug
- daemon
---
# Watcher misses working-tree edits, leaving the UI stale

A task edited on disk (CLI or editor) is sometimes never reported over `/api/events`, so the open UI
keeps showing the old state until an unrelated git operation forces a watcher pass.

## Observed

Reproduced against the live daemon serving the main checkout:

- `openplan set <id> status done` inside a worktree created ~1 min earlier — no SSE event in a 10s window.
- `git commit` in that same worktree ~2 min later — event arrives at once, carrying the earlier edit.
- Three freshly created worktrees afterwards — every edit reported immediately.

So the file write produced no fs event the watcher acted on, while a `.git/refs` write did. That
points at the watch on that worktree's `.plan/tasks` not being live at the time: either
`notifier.watch()` never succeeded for it, or the FSEvents stream lost it. `reconcile` only retries
on the next pass, which is why the commit "fixed" it. The state could not be forced again in a dozen
attempts, so it is timing-dependent and the mechanism is unconfirmed.

Ruled out: the SSE plumbing (an API `PATCH` publishes and arrives normally), the web client
(`applyChange` refreshes list and detail on `task_changed`), and the watcher thread being dead.

## How to catch it

Run a watcher build that logs each fs event, each `reconcile` with its desired and watched sets, and
each pass outcome. The next occurrence then shows directly whether the event arrived, whether the
path was in the watch set, and what the snapshot returned.

## Separate defect found while investigating

`snapshot` (`crates/op-watch/src/lib.rs`) opens every live worktree's store and propagates the first
error, so one unreadable worktree costs change detection for every branch: `attempt_pass` retries
every 400ms while emitting nothing at all, and `Watcher::start` fails outright, leaving the daemon
with `watch disabled` for its whole life. With `.plan` present on every branch here the state only
occurs during a checkout, which the retry heals on its own — so this does not explain the report
above, but the blast radius is worth removing: make an unreadable worktree a per-branch condition,
carrying that branch's cells over from the previous snapshot and letting the others diff normally.
