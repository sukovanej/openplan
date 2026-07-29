---
status: backlog
created: 2026-07-16T10:30:47Z
---
# Reusable ⌘K palette component + task search (CLI + web)

Ship two things over one shared core:

1. A **reusable ⌘K palette component** in the web UI — a generic overlay
   primitive that later commands (the general command interface) plug into.
2. **Task search** as this palette's first consumer, plus an `oplan search`
   CLI that reuses the same matching core.

## Reusable palette component

A generic, consumer-agnostic overlay. It owns the chrome; consumers own the
content.

- **Component owns:** the floating overlay, the text input, selection state and
  keyboard handling (↑/↓ to move, ↵ to select, Esc to close), and rendering the
  result list.
- **Consumer contract (item provider):** a consumer supplies
  `query → items`, a per-row renderer, and an `onSelect` handler. The component
  is otherwise blind to what the items *are*.
- **Home concept:** the palette has a "home" that the future general command
  interface will populate. That command interface is a **separate later task** —
  it is not built here; only the seam for it is.
- Built on the existing web-ui keyboard-shortcut architecture.

## Entry points

- **`/`** opens the palette defaulted into the **search** consumer (fast path).
- **⌘K** opens the palette **home**. In this cut home has only the search
  consumer registered, so ⌘K effectively lands on search too; once the general
  command interface ships, ⌘K opens that instead.

## Search consumer + matching core

- **Scope:** title (H1) + full body (all sections) + frontmatter fields
  (`status`, `parent`, `deps`).
- **Semantics:** case-insensitive **substring/literal**. No fuzzy, no relevance
  ranking — just a stable order. A task matches if the query is a substring of
  any searched field.
- **Branches:** search **all branches**, reusing the existing cross-branch
  aggregation. Results carry their source branch.
- Empty query returns no results.
- Result rows show status + title; **no match highlighting**.
- Selecting a result navigates to / selects that task in the table.

## CLI — `oplan search <query>`

- Prints matching tasks as `id / status / title`, mirroring `oplan list`.
- `--json` emits `[{id,title,status,parent?,branch}]`.
- Reuses the same matching core as the search consumer so behavior can't drift.

## Implementation notes

### Matching core lives in Rust, not the web client

The web client has **no task body**: `GET /api/tasks` returns `TaskListItem`
(`id`/`title`/`status`/`branches`) and the cross-branch `Matrix` cell is a
`TaskSummary` — both title/status only. Body + frontmatter exist only in
`TaskView` behind per-id `GET /api/tasks/{id}`. So body/frontmatter search
cannot be a client-side filter and cannot run off the current matrix summaries.

Put the matching core in **`op-index`**, which already reads every branch's task
blobs (`repo.branch_task_blobs`, blob cache keyed by OID). Add a search function
that walks those per-branch blob `Version`s, parses each blob's full markdown
(title + all sections + frontmatter), and tests the case-insensitive substring
predicate. Reuse the existing blob cache rather than re-reading git. Each result
row carries its source branch.

### Two consumers of that one core

- **CLI:** new `Command::Search { query, --json }` in `op-cli/src/main.rs`,
  output mirroring `list_all_branches` (`branch / id / status / title`).
- **Server:** new route `GET /api/search?q=…` in `op-server`, run via
  `spawn_blocking` after `index.rebuild` (same shape as `list_tasks`), returning
  the result rows (a `TaskListItem`-like shape carrying `branch`).

### Web palette on the existing keys architecture

- Reuse `web/.../lib/keys` (`bindings`, `Scope`, `OverlayControls`,
  `useKeyboard`). Add `global`-scope bindings: ⌘K → open palette home, `/` →
  open palette in search. Add `overlay`-scope keys for the palette (↑/↓/↵/Esc).
- **Architectural change — named-overlay registry.** The keys model currently
  assumes one overlay (help): a single `overlayOpen: boolean` in `useKeyboard`,
  one `overlay: OverlayControls` on `RunContext`, and one `"overlay"` value in
  `Scope`. A reusable palette that coexists with help means keying all three by
  a closed overlay-name union instead:
  - `export type OverlayName = "help" | "palette"`, and
    `Scope = "global" | "list" | "detail" | OverlayName` (names are their own
    scopes, so help vs palette keys can't cross-fire).
  - `useKeyboard` exposes `activeOverlay: OverlayName | null` (one open at a
    time) replacing `overlayOpen`; `activeScopes` returns
    `activeOverlay ? [activeOverlay] : ["global", route]`.
  - `ctx.overlay` becomes `ctx.overlay(name: OverlayName)` → that overlay's
    open/close/toggle. Help's bindings switch to `ctx.overlay("help")`, the
    palette's Esc/nav use `ctx.overlay("palette")`.
  - `App.tsx` renders each overlay off the shared state
    (`open={activeOverlay === "…"}`). Adding a future overlay = one new
    `OverlayName` member, which the compiler forces every site to handle.
  - Touches `keys/types.ts` + `dispatcher.ts` + `use-keyboard.ts`; the
    behavioral risk is help regressing, so re-verify `?`/Esc after.
- Palette item-provider contract `{ query → items, renderRow, onSelect }`. The
  search provider fetches `GET /api/search?q=` (debounced), renders status +
  title rows, and `onSelect` navigates via `ctx.navigate`/`row-cursor` to the
  task (honoring its branch).
- v1 does not live-update open results over SSE; a new query re-fetches.

## Out of scope

- The general command interface / any non-search commands (later task; only the
  palette seam is built here).
- Fuzzy matching and relevance ranking.
- Match highlighting / snippets.
- Saved searches, search history, field-filter syntax (`status:`), regex.
