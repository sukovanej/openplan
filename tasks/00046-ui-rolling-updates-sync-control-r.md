---
status: backlog
created: 2026-07-26T15:40:55Z
parent: ./00039-continuous-changes-accumulation-v.md
dependencies:
- ./00040-daemon-ambient-writer-accumulate.md
- ./00042-publish-fast-forward-main-to-the.md
---
# UI: rolling-updates sync control + review popover

**Phase 7** (final) of the rolling-updates plan
([[./00023-design-a-continuous-changes-accu.md]]). The header sync control + the
"Rolling updates" review popover. Mockup of every state/layout:
[../assets/sync-button-options.html](../assets/sync-button-options.html).

## Wiring (existing frontend)

React + Effect Schema; SSE via `EventSource("/api/events")`
(`web/packages/app/src/lib/realtime.ts`); header already renders
`ConnectionStatus` (emerald/amber convention) + `ThemeToggle`.

- **`SyncStatus` schema** in `lib/api.ts` mirroring Phase 2's type
  ([[./00040-daemon-ambient-writer-accumulate.md]]): `{ state, pending: TaskChange[],
  conflicted: string[] }`. Fetch `GET /api/sync`.
- **Live updates** over the existing SSE stream: extend the `ChangeEvent` union
  in `lib/events.ts` with a sync event and invalidate/refetch sync status on it
  (Phase 2 pushes it on the same channel).

## `SyncControl` (header, beside ConnectionStatus / ThemeToggle)

One color-coded icon + count pill; state -> color + icon + tooltip, reusing
`emerald`=good / `amber`=warn and adding blue for "action available":
- *In sync* — muted, check: "nothing to publish."
- *Pending (N)* — blue, count pill: "N changes ready to publish to main."
- *Syncing* — blue spinner: publish or auto-refresh running.
- *Blocked* — amber, warning: a refresh conflict paused sync.
- *Offline* — dim, disabled: daemon down (reuse `useConnection`).

## "Rolling updates" review popover (on click — never publishes blind)

- Lists pending ambient changes (task title + what changed) from `pending`;
  conflicting ones (`conflicted`) flagged amber.
- Primary **"Publish N to main"** action **inside** the popover -> `POST
  /api/publish` (Phase 5 [[./00042-publish-fast-forward-main-to-the.md]]); shows
  `Syncing` while in flight; on the `409` non-FF error surfaces "main moved —
  refreshing, retry" rather than a hard failure.
- In *Blocked* state, replace Publish with a **"Resolve conflict"** action that
  surfaces the conflicted tasks; this popover is where the conflict-hold state is
  read and resolved.

## Scope

Frontend only, against the Phase 2 (`/api/sync` + SSE) and Phase 5
(`/api/publish`) surfaces. No daemon changes.

## Verify

Component tests in `web/packages/app/tests/` + interactive check (headless
Chrome over CDP, per the repo's web-UI verify recipe):
- each of the 5 states renders the right color/icon/tooltip; Offline follows
  `useConnection`.
- popover lists pending changes; conflicts flagged amber.
- Publish calls `/api/publish` and returns to *In sync* on success; a `409`
  surfaces the retriable message.
- Blocked shows Resolve conflict instead of Publish.
- an SSE sync event updates the control without a manual refetch.
