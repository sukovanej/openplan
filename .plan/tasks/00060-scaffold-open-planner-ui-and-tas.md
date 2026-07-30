---
status: backlog
created: 2026-07-30T18:53:03Z
parent: ./00059-web-ui-extract-reusable-componen.md
---
# Scaffold @open-planner/ui and task-ui; move tokens and leaf primitives

Stand up both packages and move the components that carry no dependency on the
app — the ones a move alone finishes.

## Scaffold

- `web/packages/ui` and `web/packages/task-ui`, each with `package.json`
  (`private`, `type: module`, `main`/`exports` pointing at `./src/index.ts`),
  `tsconfig.json` extending `tsconfig.base.json` with `composite: true`, and a
  `vitest.config.ts` using `happy-dom` for `ui`.
- Add both to the root `tsconfig.json` references and to `packages/app`'s
  project references; add `@open-planner/ui` and `@open-planner/task-ui` as
  `workspace:*` deps of the app, and `@open-planner/ui` as a dep of `task-ui`.
- New deps come from the workspace catalog, not fresh version ranges.

## Move into `@open-planner/ui`

- `cn` from `src/lib/utils.ts`.
- `styles.css`: the `:root` / `.dark` token blocks and `@theme inline` from
  `src/index.css`. Add semantic `danger`, `success` and `accent-line` tokens plus
  the six per-status colours, and point the existing components at them in the
  step that touches each. The app's `index.css` keeps the Tailwind and typography
  imports, the `dark` variant, the base layer, and gains `@source` entries for
  `../../ui/src` and `../../task-ui/src`.
- `Skeleton` (from `components/ui/skeleton.tsx`), and `SkeletonList` /
  `EmptyState` from the shapes in `components/states.tsx`.
- `MetaLine`, `MetaItem` from `components/task-meta.tsx` — the surrounding
  `TaskTimes` stays behind for now.
- `TimeAgo` from `components/time-ago.tsx`, with `relativeTime` and
  `absoluteTime` from `src/lib/format.ts`. `errorText` stays in the app; it
  formats an Effect error, not a time.
- `Kbd` from the `<kbd>` in `components/help-overlay.tsx:120-129`, taking the
  `KEY_LABELS` / `MODIFIER_LABELS` platform formatting with it.
- `Tag`: the bordered, dashed-when-dirty, monospace shape from
  `components/branch-tag.tsx`, with the branch semantics left behind.
- `CountPill`: the pill from `routes/detail.tsx:293-295`, replacing the unused
  `components/ui/badge.tsx`.
- `Prose`: the `proseColors` / `proseSpacing` / `proseCode` / `proseTaskList` /
  `proseTable` strings from `components/task-body.tsx:17-40` behind a component
  that wraps its children. Markdown parsing stays in the app for now.

Delete `components/ui/card.tsx` and `components/ui/badge.tsx`; nothing imports
them. Repoint or drop `components.json`, whose shadcn aliases no longer describe
the tree.

## Tests

Move `tests/format.test.ts` to the `ui` package alongside the formatters. Add
render tests for `Tag`, `CountPill` and `EmptyState`.

## Verify

`pnpm typecheck`, `pnpm lint`, `pnpm test`, `pnpm format:check`, plus `mise run
rebuild` and an interactive pass confirming the list and detail views are
unchanged — particularly that no Tailwind class was purged out of the new
package.
