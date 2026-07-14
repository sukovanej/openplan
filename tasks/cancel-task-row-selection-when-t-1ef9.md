---
status: backlog
---
# Cancel task-row selection when the mouse hovers over the list

## Problem

The task list keeps a single keyboard "row cursor" (`rowCursor`,
`web/packages/app/src/lib/row-cursor.ts`) whose selected row renders as
`bg-accent` (`web/packages/app/src/routes/list.tsx:112`). A mouse-hovered row
uses `hover:bg-accent` — the *same* background.

Today the cursor is set only by clicking a row (`onClick={() =>
rowCursor.focus(index)}`, `list.tsx:107`; arrow-key navigation is not wired
yet). Because `rowCursor` is a module-level singleton, that selection persists
when you open a task and come back to the list. So on return, the previously
selected row still shows `bg-accent`, and as soon as the mouse hovers a
*different* row a second identical `bg-accent` appears — two rows look "current"
at once with no way to tell which.

## Goal

While the mouse is moving over the task list, dismiss the keyboard row-cursor so
only the hovered row is highlighted. A later keyboard action re-establishes the
cursor. "Last input modality wins."

## Design

1. Add `clear()` to `RowCursorStore` (`row-cursor.ts`) that sets `index: -1`
   while keeping `ids` unchanged, backed by a pure `cleared(state)` reducer
   alongside `withRows`/`moved`/`focused`. Note: `focus(-1)` will not work —
   `clampIndex` clamps to `[0, count-1]`, so an explicit cleared state (matching
   `emptyCursor`'s `index: -1`) is required.

2. In `list.tsx`, attach `onMouseMove` to the `role="grid"` container
   (`list.tsx:49`) that calls `rowCursor.clear()`, guarded by
   `if (index !== -1)` so it writes to the store at most once per dismissal
   (`mousemove` fires continuously). Use `onMouseMove` (mouse-only), not
   pointer/touch events — hover is a mouse concept.

3. Keep the existing `onClick` focus and the `<Link>` overlay. The approach move
   toward a row clears any stale selection; the click then re-selects that row
   and navigates to its detail page, so returning to the list shows where you
   were until the next mouse move. No fight in practice.

No styling change needed: once hover clears the selection, at most one row shows
`bg-accent`.

## Done when

- With a row selected, moving the mouse over the list clears the selection (no
  `aria-selected` row; grid's `aria-activedescendant` unset) and only the
  hovered row shows `bg-accent`.
- Moving the mouse when nothing is selected does not write to the store
  (guarded).
- The single-highlight invariant holds: the list never shows two `bg-accent`
  rows at once.

## Tests (`web/packages/app/tests/`)

- Extend `row-cursor.test.ts`: `clear()` sets `index` to `-1` and preserves
  `ids`; `clear()` on an already-cleared cursor is idempotent (no listener
  churn).
- If a component/DOM harness is available, assert a `mousemove` on the grid
  clears an active selection; otherwise cover the store behavior and wire the
  handler by inspection.

## Refs

- `web/packages/app/src/lib/row-cursor.ts` — store + reducers (`clampIndex`,
  `emptyCursor`, `focus`, `moveBy`).
- `web/packages/app/src/routes/list.tsx:49` (grid container), `:107` (onClick
  focus), `:112` (accent / hover styling).
