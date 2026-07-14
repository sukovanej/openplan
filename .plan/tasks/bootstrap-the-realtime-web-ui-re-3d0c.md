---
status: todo
---
# Bootstrap the realtime web UI (React + Effect + shadcn)

## Goal
Stand up the real web frontend that `op-server` embeds (replacing the `web/index.html`
placeholder from [[init-workspace-a1e0]]): a minimal, realtime SPA a human watches while
agents change tasks live (§9). This init delivers two read views end-to-end — a task **list**
and a single-task **detail with a markdown viewer** — plus the architecture the rest of the UI
grows on: an Effect-based data layer, a shadcn/Tailwind component base, and a clean realtime
channel wired all the way from the daemon to React. Mutations (create/edit/status/reorder) and
the branch/matrix/presence views (§9) are explicit follow-ups.

## Stack (proposed)
- **Vite + React + TypeScript** SPA. Build output is embedded by `op-server` (`rust-embed`), so
  the daemon still ships as one static binary (§10).
- **Effect v4** (`effect`) as the data/effect layer: API access and the realtime stream are
  `Effect` services exposed as `Layer`s over one `ManagedRuntime`; React binds to them via
  `@effect-atom/atom-react` (atoms derived from services, so components stay declarative and the
  realtime channel drives invalidation, not `useEffect` soup).
- **shadcn/ui** (Radix + Tailwind, components copied into `web/src/components/ui`) for the UI
  primitives; `components.json` + Tailwind configured via the shadcn init.
- **React Router** for the two routes (`/`, `/task/:id`). (TanStack Router is the alternative —
  React Router chosen for minimal footprint at this size.)
- **Markdown**: `react-markdown` + `remark-gfm` (task bodies use GFM tables and `- [ ]`
  checklists — see any task file), styled with `@tailwindcss/typography` to match shadcn.

## Project layout
Frontend source lives in `web/` as a self-contained Vite project; the build emits to
`web/dist/`, which becomes the embed root:
```
web/
  index.html            # Vite entry (was the embedded placeholder; now the source template)
  package.json          # pnpm/npm project — NOT embedded
  vite.config.ts        # dev proxy + build.outDir = "dist"
  tailwind.config.ts  components.json  tsconfig.json
  src/
    main.tsx  App.tsx  routes/{list,detail}.tsx
    lib/api.ts          # Effect Service: HttpClient over /api/tasks(/:id)
    lib/events.ts       # Effect Stream over /api/events (SSE) -> ChangeEvent
    lib/runtime.ts      # ManagedRuntime + Layers; atoms
    components/ui/*      # shadcn components
  dist/                 # vite build output; rust-embed folder (committed placeholder keeps
                        #   `cargo build` working before any frontend build)
```
- Repoint `op-server`'s `#[folder = "../../web"]` → `#[folder = "../../web/dist"]`.
- Commit a minimal `web/dist/index.html` placeholder so a Rust-only `cargo build` still embeds a
  valid (if bare) SPA and `rust-embed` never fails on a missing folder; a real `vite build`
  overwrites it.
- `.gitignore`: `web/node_modules/`; keep `web/dist/` **tracked** (single-binary builds must not
  require Node — the committed build is what `cargo install` embeds).

## Data layer (Effect)
- `TasksApi` service wraps `@effect/platform` `HttpClient`, decoding responses with
  `Schema` that mirror the `op-api` DTOs from [[task-crud-6e8b]]:
  - `list: Effect<TaskSummary[]>`        ← `GET /api/tasks`
  - `get(id): Effect<TaskView>`          ← `GET /api/tasks/:id`
  (Write methods exist in the API already but their UI is out of scope here.)
- Errors are typed (`TaskNotFound`, `RequestError`) and rendered as inline states, never thrown
  past the runtime.
- React reads via atoms: `tasksAtom` (list), `taskAtom(id)` (detail). Loading/error/success are
  atom states, so views are pure functions of atom result.

## Realtime (the clean channel)
The daemon currently has no server→client push: [[init-workspace-a1e0]]'s watcher feeds an
in-process `mpsc<ChangeEvent>` in `serve.rs` that goes nowhere. This task adds the minimal,
complete path so realtime is real rather than mocked:
- **Server**: add `GET /api/events` to `op-server` — an SSE (`text/event-stream`) response
  streaming `op_api::ChangeEvent` as JSON. Back it with a `tokio::sync::broadcast<ChangeEvent>`
  in `AppState`; bridge the watcher's `mpsc` into the broadcast in `serve.rs`, and have the
  create/patch/delete handlers publish to it too (so UI- and CLI/agent-driven writes both fan
  out to every connected client). Auto-reconnect is free with SSE.
- **Client**: `lib/events.ts` exposes the SSE feed as an Effect `Stream<ChangeEvent>`
  (`EventSource` wrapped in a `Stream`, retried with backoff). A subscriber maps each event to
  the atoms it invalidates (`TaskChanged{id}` → refetch that task + the list; coarse events →
  refetch the list). Editing a task via `oplan` or an agent visibly updates the open UI.
- **SSE over WebSocket** for v1: the UI's realtime need is one-directional (reads are global,
  writes go through REST — §7.1), so SSE is the simpler, self-healing fit. The spec's WS (§9,
  §10) is reserved for later bidirectional needs (live presence/cursors).

## Dev workflow
- `vite dev` (5173) with a proxy for `/api`, `/api/events`, `/health` → the daemon
  (`http://localhost:7373`, the `DEFAULT_PORT`; override via env).
- Production: `vite build` → `web/dist`, then `cargo build`; `oplan server start` serves the
  embedded SPA and the API from one port. Add the two-step build to the README build essentials.

## Acceptance criteria
- [ ] `cd web && <pm> install && <pm> run build` produces `web/dist/{index.html, assets/*}`;
      `cargo build` embeds it and `oplan server start` serves the SPA at `/` with the API.
- [ ] `/` lists tasks from `GET /api/tasks` (title + status badge), each linking to its detail.
- [ ] `/task/:id` shows the task: title, status, and the body rendered by the markdown viewer
      (GFM tables + `- [ ]` checklists render); a missing id shows a clean not-found state.
- [ ] Realtime: with the UI open, `oplan set <id> status done` (or a `PATCH`) updates the list
      and the open detail **without a manual refresh**, driven by an `/api/events` SSE message.
- [ ] `GET /api/events` streams `ChangeEvent` JSON; the server tests cover a publish → SSE
      delivery roundtrip (extend the existing `tower::ServiceExt` suite).
- [ ] Rust-only `cargo build` (no Node) still compiles and embeds the committed `dist`
      placeholder; `cargo build`, `cargo test`, `cargo fmt --check`, `cargo clippy -- -D warnings`
      all pass.
- [ ] Frontend `tsc --noEmit` and lint are clean; no `useEffect`-based data fetching (data and
      realtime both flow through Effect atoms).

## Out of scope (follow-up tasks)
- All write UI: create/edit task, inline section edit, status change, drag-to-reorder (`rank`),
  claim/release — §9 interactions. The REST + `oplan` write paths already exist ([[task-crud-6e8b]]).
- Branch-aware matrix view, worktree swimlanes, and presence dots (§9) — needs `/api/matrix`
  and presence, and richer events.
- **Event fidelity**: real debounced `TaskChanged`/ref/presence events from `op-watch` (still a
  skeleton `TODO` emitting coarse `RefMoved` on any modify) — the UI degrades gracefully by
  refetching on coarse events until then.
- WebSocket transport; multi-project aggregation across stores.

## Notes
- Keep it minimal: two read views, one realtime channel, one component base. Everything else in
  §9 layers on afterward.
- The frontend framework was left TBD by [[init-workspace-a1e0]]; this task resolves it
  (Vite/React/TS/Effect/shadcn) and moves the embed root to `web/dist`.
- Reuse the `op-api` DTO shapes as the Schema source of truth — the wire contract is
  Rust-owned; the TS `Schema` mirrors it and fails loudly on drift.
