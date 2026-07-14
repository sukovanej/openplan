---
status: done
---
# Bootstrap the realtime web UI (React + Effect + shadcn)

## Goal
Stand up the real web frontend that `op-server` embeds (replacing the `web/index.html`
placeholder from [[init-workspace-a1e0]]): a minimal, realtime SPA a human watches while
agents change tasks live (§9). This init delivers two read views end-to-end — a task **list**
and a single-task **detail with a markdown viewer** — inside a **TypeScript monorepo** set up
the way Effect projects are, so later packages (a generated API client, shared UI, shared
domain schemas) drop in without re-plumbing. Mutations and the branch/matrix/presence views
(§9) are explicit follow-ups.

## Monorepo (`web/`)
`web/` becomes a **pnpm workspace**, the TS-side mirror of the Rust cargo workspace under
`crates/`. Layout and config follow the Effect monorepo template, adapted (linter → oxc, no
publishing):
```
web/
  pnpm-workspace.yaml        # packages: ["packages/*"]  (+ pnpm catalog for shared versions)
  package.json               # root scripts: `-r` fan-out (build/lint/format/typecheck/test)
  tsconfig.base.json         # shared compiler options; strict
  tsconfig.json              # solution file: project references over every package
  vitest.shared.ts           # shared vitest config           }
  vitest.workspace.ts        # discovers per-package projects  } Effect-template layout
  .oxlintrc.json             # oxlint rules (replaces eslint.config.mjs)
  dprint.json                # formatter config
  packages/
    app/                     # THE app this task ships — Vite + React + Effect + shadcn SPA
      index.html  vite.config.ts  tailwind.config.ts  components.json  tsconfig.json
      src/{main,App}.tsx  src/routes/{list,detail}.tsx
      src/lib/{api,events,runtime}.ts   src/components/ui/*
      dist/                  # vite output; the rust-embed root (committed placeholder inside)
```
- **Repoint `op-server`**: `#[folder = "../../web"]` → `#[folder = "../../web/packages/app/dist"]`.
- Commit a minimal `packages/app/dist/index.html` placeholder so a Rust-only `cargo build` still
  embeds a valid SPA and `rust-embed` never fails on a missing folder; a real build overwrites it.
- `.gitignore`: `web/**/node_modules/`; keep `packages/app/dist/` **tracked** — single-binary
  builds (`cargo install`) must embed the UI without Node in the loop.
- **Future packages (design for, don't build now):** `packages/domain` (Effect `Schema` mirroring
  the `op-api` DTOs, shared by app + client), `packages/api-client` (the generated Effect client
  off the Rust OpenAPI — deferred), `packages/ui` / `packages/config` (shared shadcn + tsconfig /
  oxlint / tailwind presets). Each is just another `packages/*` entry + project reference.

## Tooling (Effect-recommended, oxc-first)
The Effect template ships pnpm + TS project references + Vitest + **ESLint & dprint** +
Changesets + Nix. We keep the Effect-native pieces and swap the linter to **oxc** per the repo's
Rust-native bias:
- **Package manager**: pnpm workspaces + **catalog** so shared dep versions (react, effect,
  vite, vitest) are pinned once.
- **TypeScript**: `tsconfig.base.json` (strict) + per-package `tsconfig.json` with **project
  references** and path aliases; root `tsconfig.json` is the solution file. `tsc -b` typechecks
  the graph in dependency order.
- **Tests**: **Vitest** + **`@effect/vitest`** (`it.effect` / `it.scoped`, `TestClock`) —
  `vitest.shared.ts` + `vitest.workspace.ts` per the template. Ships with real tests: the
  `TasksApi` decode path against a mocked `HttpClient`, and the SSE→invalidation reducer.
- **Lint**: **oxlint** (`.oxlintrc.json`) — Rust-based, ~zero-config, ~50–100× ESLint; the
  correctness + typescript + react + react-hooks plugins on.
- **Format**: **dprint** (`dprint.json`), the Effect-standard formatter. (oxfmt is the all-oxc
  alternative once stable — noted, not adopted yet.)
- **Root scripts** (`pnpm -r --filter`): `build`, `typecheck`, `lint`, `format`(`:check`),
  `test`. A Turborepo task graph is optional and can layer on later for caching.
- **Dropped from the template**: Changesets (nothing is published — the app is embedded) and the
  Nix flake (this repo pins toolchains via `rust-toolchain.toml` + pnpm, not Nix).

## App stack (`packages/app`)
- **Vite + React + TypeScript** SPA; build embedded by `op-server` so the daemon stays one
  static binary (§10).
- **Effect v4** (`effect`) data/effect layer: API access and the realtime stream are `Effect`
  services exposed as `Layer`s over one `ManagedRuntime`; React binds via
  `@effect-atom/atom-react` (atoms derived from services — declarative views, realtime drives
  invalidation, not `useEffect` soup).
- **shadcn/ui** (Radix + Tailwind, components copied into `src/components/ui`); `components.json`
  + Tailwind via shadcn init.
- **React Router** for `/` and `/task/:id` (TanStack Router is the alternative; React Router for
  minimal footprint here).
- **Markdown**: `react-markdown` + `remark-gfm` (bodies use GFM tables and `- [ ]` checklists),
  styled with `@tailwindcss/typography` to match shadcn.

## Data layer (Effect)
- `TasksApi` service wraps `@effect/platform` `HttpClient`, decoding with `Schema` that mirror
  the `op-api` DTOs from [[task-crud-6e8b]] (these Schemas are what later moves to
  `packages/domain`):
  - `list: Effect<TaskSummary[]>`  ← `GET /api/tasks`
  - `get(id): Effect<TaskView>`    ← `GET /api/tasks/:id`
- Errors are typed (`TaskNotFound`, `RequestError`) and rendered as inline states, never thrown
  past the runtime.
- React reads via atoms: `tasksAtom` (list), `taskAtom(id)` (detail); loading/error/success are
  atom states, so views are pure functions of the atom result.

## Realtime (the clean channel)
The daemon has no server→client push today: [[init-workspace-a1e0]]'s watcher feeds an
in-process `mpsc<ChangeEvent>` in `serve.rs` that goes nowhere. This task adds the minimal,
complete path so realtime is real, not mocked:
- **Server**: add `GET /api/events` to `op-server` — SSE (`text/event-stream`) streaming
  `op_api::ChangeEvent` as JSON, backed by a `tokio::sync::broadcast<ChangeEvent>` in
  `AppState`. Bridge the watcher's `mpsc` into the broadcast in `serve.rs`, and have the
  create/patch/delete handlers publish too, so UI- and CLI/agent-driven writes both fan out to
  every connected client. SSE auto-reconnects.
- **Client**: `lib/events.ts` exposes the feed as an Effect `Stream<ChangeEvent>` (`EventSource`
  wrapped in a `Stream`, retried with backoff); a subscriber maps each event to the atoms it
  invalidates (`TaskChanged{id}` → that task + the list; coarse events → the list). Editing a
  task via `oplan` or an agent visibly updates the open UI.
- **SSE over WebSocket** for v1: the UI's realtime need is one-directional (reads global, writes
  via REST — §7.1), so SSE is the simpler, self-healing fit. The spec's WS (§9/§10) is reserved
  for later bidirectional needs (presence/cursors).

## Dev workflow
- `pnpm --filter app dev` → Vite dev server (5173) proxying `/api`, `/api/events`, `/health` to
  the daemon (`http://localhost:7373`, `DEFAULT_PORT`; override via env).
- Production: `pnpm -r build` (builds `packages/app` → `dist`), then `cargo build` embeds it;
  `oplan server start` serves the SPA + API from one port. Add the two-step build to the README
  build essentials.

## Acceptance criteria
- [ ] `cd web && pnpm install && pnpm -r build` produces `packages/app/dist/{index.html,assets/*}`;
      `cargo build` embeds it and `oplan server start` serves the SPA at `/` with the API.
- [ ] `/` lists tasks from `GET /api/tasks` (title + status badge), each linking to its detail.
- [ ] `/task/:id` renders title, status, and the body via the markdown viewer (GFM tables +
      `- [ ]` checklists render); a missing id shows a clean not-found state.
- [ ] Realtime: with the UI open, `oplan set <id> status done` (or a `PATCH`) updates the list
      and open detail **without a manual refresh**, driven by an `/api/events` SSE message.
- [ ] `GET /api/events` streams `ChangeEvent` JSON; server tests cover publish → SSE delivery
      (extend the existing `tower::ServiceExt` suite).
- [ ] Monorepo quality gates pass: `pnpm -r typecheck` (`tsc -b`), `pnpm lint` (oxlint),
      `pnpm format:check` (dprint), `pnpm -r test` (Vitest + `@effect/vitest`, with the two tests
      above). No `useEffect`-based data fetching.
- [ ] Rust-only `cargo build` (no Node) compiles and embeds the committed `dist` placeholder;
      `cargo build`, `cargo test`, `cargo fmt --check`, `cargo clippy -- -D warnings` all pass.

## Out of scope (follow-up tasks)
- The future packages themselves: `packages/domain`, the generated `packages/api-client`, shared
  `ui`/`config` — this task only lays the workspace so they slot in.
- All write UI: create/edit, inline section edit, status change, drag-to-reorder, claim/release
  (§9). REST + `oplan` write paths already exist ([[task-crud-6e8b]]).
- Branch-aware matrix view, worktree swimlanes, presence dots (§9) — need `/api/matrix`,
  presence, and richer events.
- **Event fidelity**: real debounced `TaskChanged`/ref/presence events from `op-watch` (still a
  skeleton `TODO` emitting coarse `RefMoved` on any modify) — the UI degrades by refetching on
  coarse events until then.
- WebSocket transport; Turborepo caching; multi-project aggregation.

## Notes
- Structure and config track the **Effect monorepo template** (pnpm, project references,
  `vitest.shared.ts`/`vitest.workspace.ts`); the one deliberate divergence is **oxc** for
  linting instead of ESLint, matching this repo's Rust-native, single-fast-binary bias.
- **oxc caveat**: oxlint has no Effect-specific rules yet (the `@effect/eslint-plugin` niceties —
  dprint-as-lint, no-barrel-import — don't exist there); dprint covers formatting, and if
  Effect-aware lint rules become necessary later, add ESLint as a second, CI-only pass.
- Reuse the `op-api` DTO shapes as the `Schema` source of truth — the wire contract is
  Rust-owned; the TS `Schema` mirrors it and fails loudly on drift. Those Schemas are the seed of
  `packages/domain`.
- The frontend framework, left TBD by [[init-workspace-a1e0]], is resolved here; the embed root
  moves from `web/` to `web/packages/app/dist`.
