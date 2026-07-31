---
status: backlog
created: 2026-07-30T18:53:00Z
---
# Web UI: extract reusable component packages (ui + task-ui)

The web SPA keeps every component in `web/packages/app`, and the same markup is
now spelled several times over. Extract it into two workspace packages: a
domain-free primitive layer and a task-shaped layer on top of it, leaving the app
as routes plus data wiring.

## What is duplicated today

| Pattern | Call sites |
| --- | --- |
| status icon + id + truncated title | `routes/list.tsx:181-194`, `routes/detail.tsx:75-77`, `routes/detail.tsx:339-341`, `routes/detail.tsx:150-160`, `components/task-body.tsx:79-99` |
| parent link (icon + id + truncated title) | `routes/list.tsx:198-204` and `routes/detail.tsx:197-200`, byte-identical |
| framed panel + `h-11` uppercase header | `routes/list.tsx:57-61` and `routes/detail.tsx:73-86` |
| current-row treatment | `routes/list.tsx:119-125`, `routes/detail.tsx:334-337`, `components/search-combobox.tsx:154-157` — three spellings |
| red notice surface | `components/flash.tsx:20` and `components/mutation-error.tsx:14` |
| ad-hoc button class strings | `routes/detail.tsx:210`, `routes/detail.tsx:300`, `components/help-overlay.tsx:88`, `components/theme-toggle.tsx:33` |
| raw palette instead of tokens | `status-badge`, `branch-tag`, `task-meta`, `flash`, `mutation-error`, `list` |

`components/ui/card.tsx` and `components/ui/badge.tsx` are unreferenced.

## `@open-planner/ui`

No `@open-planner/api-client`, no `react-router-dom`. Deps: `react`, `clsx`,
`tailwind-merge`, `class-variance-authority`, `lucide-react`.

`Panel` / `PanelHeader` / `PanelBody`, `Row`, `Section`, `Button`,
`IconToggleGroup`, `Dialog`, `Toast`, `Combobox` + `FuzzyText`, `Skeleton`,
`SkeletonList`, `EmptyState`, `MetaLine`, `MetaItem`, `TimeAgo`, `Prose`, `Tag`,
`CountPill`, `Kbd`, plus `cn`, `styles.css` and the time formatters.

## `@open-planner/task-ui`

Deps: `@open-planner/ui`, `@open-planner/api-client`, `react-router-dom`.

`StatusIcon`, `StatusField`, `StatusChip`, `statusLabel`, `statusOrder`,
`StatusGroupHeader`, `TaskIdentity`, `ParentLink`, `TaskTimes`, `TaskRefChip`,
`BranchTag`, `BranchBadges`, `BranchSwitcher`, `TaskBody`.

## Stays in the app

Routes, all of `src/lib` (store, api, realtime, keys, cursors, theme, metadata,
copy-target), and the adapters that bind a store to a package component:
`ConnectionStatus`, `MutationError`, `Flash`, `HelpOverlay`, `ThemeToggle`.
`components/ui/card.tsx` is deleted rather than moved.

## Constraints

- Both packages are source-only (`main: ./src/index.ts`), matching
  `@open-planner/api-client`.
- Tailwind v4 purges what it cannot read: the app's CSS needs `@source` entries
  for both package source trees.
- Colour tokens move into `@open-planner/ui`, gaining semantic `danger`,
  `success` and `accent-line` names plus the per-status colours, so no component
  ships a bare `red-500`.
- Row heights stay integer — fractional heights put borders on half device
  pixels and make selected rows shimmer on Retina.
- No behaviour or visual change in any step. Each step ends with `pnpm
  typecheck`, `pnpm lint`, `pnpm test`, `pnpm format:check` and an interactive
  pass over the list and detail views.

The four children are sequential; each leaves the app building and running.
