---
status: todo
created: 2026-07-15T14:39:35Z
---
# j/k selection resumes from the focused row

Three fixes to keyboard row navigation in the list view.

## 1. j/k resumes from the focused row

When no keyboard cursor is set (`index === -1`), pressing `j`/`k` jumps to the
first row regardless of which row is currently focused. It should instead begin
the selection from the currently focused row, if one is focused, and only fall
back to the first row when nothing is focused.

### Current behavior
- `rowCursor` (`web/packages/app/src/lib/row-cursor.ts`) tracks the selected row
  as `index`, where `-1` means "no selection".
- `list.down`/`list.up` bindings (`web/packages/app/src/lib/keys/bindings.ts`)
  call `cursor.moveBy(±1)`. From `index === -1`, `moved()` clamps to `0`, so the
  cursor always lands on the first row.
- In the list route (`web/packages/app/src/routes/list.tsx`), clicking a row
  focuses it via `rowCursor.focus(index)`, but `onMouseMove` clears the cursor,
  so after moving the mouse the next `j`/`k` restarts from the first row.

### Desired behavior
- If a row is focused when `j`/`k` is pressed, the first keypress moves relative
  to that focused row (or selects it) rather than jumping to row 0.
- If nothing is focused, keep the current fall-back to the first row.

## 2. The selected row stays on screen

Walking a long list with `j`/`k` moves the cursor past the edge of the viewport:
the selected row scrolls out of sight while the keypresses keep landing on rows
the user can no longer see.

### Current behavior
- `list.tsx` renders rows with no scroll follow-up; only the selection styling
  changes when `index` moves.
- The detail route already solves this for its subtask list
  (`web/packages/app/src/routes/detail.tsx:297`): a layout effect calls
  `scrollIntoView({ block: "nearest" })` on the ref'd active row.

### Desired behavior
- Whenever the cursor index changes, the selected row is scrolled into view,
  mirroring the detail route's `{ block: "nearest" }` approach so an
  already-visible row never scrolls.
- Applies to both ends of the list, and to any group header / sticky chrome the
  row would otherwise hide behind.

## 3. Hover and keyboard selection look the same

A hovered row and a `j`/`k`-selected row are styled differently: selection draws
the blue outline overlay plus `bg-muted/30`, hover only tints the background
(`hover:bg-muted/30` in `list.tsx`).

### Desired behavior
- A hovered row gets the same treatment as the selected row, blue border
  included, so the pointer and the keyboard mark the "current" row identically.
- Keep the height-stability property the current styling was built for: the
  bottom border goes transparent and the outline is drawn by the absolutely
  positioned `after:` overlay, so a row must not change height on hover.

## Notes
- Add a `tests/` unit test covering: first `j` from a focused row, and first `j`
  with nothing focused (first row).
