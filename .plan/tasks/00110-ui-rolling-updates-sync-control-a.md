---
status: backlog
created: 2026-09-06T12:31:44Z
parent: ./00039-continuous-changes-accumulation-v.md
dependencies:
- ./00109-rolling-updates-a-branch-with-a.md
tags:
- feature
- ui
---
# UI: rolling-updates sync control and review popover

The header sync control and the "Rolling updates" review popover, on top of the
backend in [[./00109-rolling-updates-a-branch-with-a.md]]. Mockup of every
state and layout: [../assets/sync-button-options.html](../assets/sync-button-options.html).

## Wiring

React and Effect Schema. SSE arrives over `EventSource("/api/events")`
(`web/packages/app/src/lib/realtime.ts`). The header already renders
`ConnectionStatus` and `ThemeToggle`.

- Add a `SyncStatus` schema to `lib/api.ts` that mirrors `GET /api/sync`:
  `{ state, pending: TaskChange[], conflicted: string[], worktree: string }`.
- Extend the `ChangeEvent` union in `lib/events.ts` with the sync event and
  refetch the status on it.

## `SyncControl`

One colour-coded icon and a count pill, beside `ConnectionStatus` and
`ThemeToggle`. Reuse the app's convention of `emerald` for good and `amber` for
warn, and add blue for "you can act".

- **In sync.** Muted, check icon. "Nothing to publish."
- **Pending (N).** Blue, count pill. "N changes ready to publish to main."
- **Syncing.** Blue spinner. A publish or a rebase is running.
- **Blocked.** Amber, warning icon. A conflict paused the rolling-updates branch.
- **Offline.** Dim and disabled. Reuse `useConnection`.

## Review popover

A click opens the popover. A click never publishes.

- List the pending changes: the task title and what changed.
- The primary action **"Publish N to main"** sits inside the popover and calls
  `POST /api/publish`. Show `Syncing` while the request runs. On the
  non-fast-forward refusal, show "main moved, refreshing, try again" rather than
  a hard failure.
- In the **Blocked** state, replace Publish with the conflict view: the
  conflicted task titles, the worktree path from `SyncStatus.worktree`, and the
  two commands to run there. The v1 UI does not edit the conflict. It tells the
  person where the files are.

## `BranchSwitcher`

`BranchSwitcher` on the task detail page lists a task's versions per branch, so
`openplan/rolling-updates` appears there. Give it the display name "Rolling updates" and
say the version is unpublished. Do not hide it. The UI has no worktree swimlane
today, so there is nothing else to keep it out of.

## Verify

Component tests in `web/packages/app/tests/`, plus an interactive check with
headless Chrome over CDP, per the repository's web-UI recipe.

- Each of the five states renders the right colour, icon, and tooltip. Offline
  follows `useConnection`.
- The popover lists the pending changes.
- Publish calls `/api/publish` and the control returns to In sync.
- A non-fast-forward refusal shows the retriable message.
- Blocked shows the conflicted tasks, the worktree path, and the commands.
- A sync event over SSE updates the control with no manual refetch.
- `BranchSwitcher` shows the rolling-updates branch as "Rolling updates" and marks the version
  unpublished.
