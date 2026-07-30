---
status: backlog
created: 2026-07-30T18:53:10Z
parent: ./00059-web-ui-extract-reusable-componen.md
dependencies:
- ./00061-panel-row-and-section-primitives.md
- ./00062-button-icontogglegroup-dialog-to.md
---
# Fill @open-planner/task-ui; collapse TaskIdentity and ParentLink

Fill `@open-planner/task-ui` — the components that know what a task is — and
collapse the duplication the primitives could not reach.

## Move

- `components/status-badge.tsx` whole: `StatusIcon`, `StatusField`,
  `StatusChip`, `statusLabel`, `statusOrder`, and the palette, now reading the
  per-status tokens instead of bare Tailwind scales. `statusHeaderClass` is
  absorbed by `StatusGroupHeader`.
- `StatusGroupHeader` from `routes/list.tsx:98-112`, including the unreadable
  group's own colour — a task whose status could not be read must not wear one it
  did not claim.
- `TaskTimes` from `components/task-meta.tsx`, over the package's `MetaItem`.
- `components/branch-tag.tsx`, `components/branch-badges.tsx` and
  `components/branch-switcher.tsx`, with `BranchTag` built on `Tag`.
- `components/task-body.tsx` as `TaskBody`, taking `refs` and the project
  abbreviation as props instead of reading `useAbbreviation`, and rendering into
  the package's `Prose`. `components/task-links.ts` comes with it; the
  abbreviation store stays in the app.

## Collapse

- `TaskIdentity` — status icon, id in tabular numerals, truncated title — with
  an optional trailing slot and optional fuzzy-match indices. Replaces the five
  spellings at `routes/list.tsx:181-194`, `routes/detail.tsx:75-77`,
  `routes/detail.tsx:339-341`, `routes/detail.tsx:150-160` (`ComboTaskRow`) and
  `components/task-body.tsx:79-99` (the reference chip, which keeps its own
  border and unresolved state as `TaskRefChip`).
- `ParentLink` — icon, id, truncated title, underline on hover — replacing the
  identical blocks at `routes/list.tsx:196-206` and
  `routes/detail.tsx:194-202`. The list's copy sits under a stretched link, so
  the `relative z-10` that keeps it clickable is part of the component.

Sizes differ between call sites (the detail header runs smaller than a list
row); that is a prop, not a second component.

## Left in the app

Routes, `src/lib`, and the store adapters: `ConnectionStatus`, `MutationError`,
`Flash`, `HelpOverlay`, `ThemeToggle`. After this step `src/components` holds
only those adapters.

## Tests

`tests/task-links.test.ts` moves with `task-links.ts`. Add render tests for
`TaskIdentity` (resolved, unresolved, unreadable status) and `ParentLink`.

## Verify

Checks, then interactively: list rows, subtask rows, picker rows, the detail
header and an inline task reference all render the same status icon, id and
title as before; a task with an unreadable status still shows the alert mark; a
reference the store cannot resolve still renders dashed with its key alone;
branch tags and the branch switcher are unchanged.
