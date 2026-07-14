---
status: backlog
---
# Non-blocking daemon shutdown + SSE connection status/reconnect UX

## Problem

Stopping the daemon blocks (and often fails) while any browser has the realtime
stream open. Realtime updates use **Server-Sent Events**, not WebSockets: the UI
holds `EventSource("/api/events")` (`web/packages/app/src/lib/realtime.ts:15`)
against the axum `Sse` endpoint (`crates/op-server/src/lib.rs:134-148`).

Shutdown runs through `axum::serve(...).with_graceful_shutdown(...)`
(`op-server/src/lib.rs:101-115`), which stops accepting connections but waits for
in-flight requests to finish. An SSE stream never finishes, so a connected tab
pins the process open and the CLI's `wait_until_exited` spins until its 5s
`STOP_DEADLINE` and errors (`op-cli/src/daemon.rs:16,209,296-310`).

## Scope

One unit of work across server and web. Four parts, shipped together.

### 1. Server: force-close SSE streams on shutdown (the actual fix)

Shutdown must not depend on client cooperation. In the `/api/events` handler,
race each stream against the shutdown signal and end the stream when it fires, so
the graceful-shutdown future resolves regardless of what any client does.

- Give the SSE stream access to the shutdown `Notify` (already in `AppState`,
  used by `admin_shutdown`, `op-server/src/lib.rs:124-132`).
- `select!` the broadcast stream against `shutdown.notified()`; on shutdown,
  emit a final `DaemonStopping` event as best-effort, then end the stream.
- Result: `oplan daemon stop` returns promptly with no open-tab dependency and no
  reliance on the 5s deadline.

### 2. Server + shared schema: `DaemonStopping` termination event

Add a variant to the wire enum so the UI can distinguish an intentional stop from
a crash or network drop. Keep the two hand-mirrored definitions in sync:

- Rust: `ChangeEvent` in `crates/op-api/src/lib.rs:181-187`.
- TS mirror: `web/packages/app/src/lib/events.ts:3-18`.

This event is best-effort UX signal only — it is NOT the shutdown mechanism
(part 1 is). If it never reaches a client, shutdown still completes.

### 3. Web: connection-status badge

Replace the static `realtime` text in the top bar (`App.tsx:13`,
`<span className="text-muted-foreground text-xs">realtime</span>`) with a colored
status `Badge` reflecting the live connection state. No status is surfaced today;
`onopen`/`onerror` are not wired up (`realtime.ts` has only `onmessage`).

- Use the existing `Badge` component (`web/packages/app/src/components/ui/badge.tsx`),
  following the `StatusBadge` color pattern
  (`web/packages/app/src/components/status-badge.tsx` — `bg-emerald-600` /
  `bg-blue-600` etc. with `border-transparent text-white`).
- States and colors:
  - **Live** — stream open (`onopen`): green (`bg-emerald-600`).
  - **Reconnecting…** — unexpected drop (`onerror`): amber (e.g. `bg-amber-500`).
  - **Daemon stopped** — received `DaemonStopping` before the drop: muted/gray
    (`variant="outline"` / `text-muted-foreground`).
- The badge reads connection state from a small store the realtime layer updates,
  so `realtime.ts` and the header component stay decoupled.

### 4. Web: reconnect + resync

- Replace reliance on `EventSource`'s built-in auto-reconnect with explicit
  control so status and backoff are observable.
- **Clean stop backs off:** after `DaemonStopping`, show "Daemon stopped" and
  poll slowly (~10s). An unexpected drop reconnects aggressively with backoff
  (~1→2→4→8s, capped).
- **Full refetch on every (re)connect:** on `onopen`, trigger a full refetch via
  the existing "refetch everything" path (`refreshVisible()` /
  `applyChange` `ref_moved`, `web/packages/app/src/lib/events.ts:26-44`) so any
  events missed while disconnected are recovered.

## Acceptance

- With a browser tab open on the UI, `oplan daemon stop` returns promptly and
  does not hit the 5s `STOP_DEADLINE`.
- The top-bar badge (replacing the old `realtime` text) reflects connection
  state: green "Live" when up, muted "Daemon stopped" on a clean stop, amber
  "Reconnecting…" on an unexpected drop.
- Restarting the daemon: the UI reconnects on its own and its data is correct
  (reflects changes made while it was down).
- `cargo build && cargo test && cargo fmt --check && cargo clippy -- -D warnings`
  pass; web builds.
