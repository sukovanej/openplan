---
status: todo
---
# j/k selection resumes from the focused row

When no keyboard cursor is set (`index === -1`), pressing `j`/`k` jumps to the
first row regardless of which row is currently focused. It should instead begin
the selection from the currently focused row, if one is focused, and only fall
back to the first row when nothing is focused.

## Current behavior
- `rowCursor` (`web/packages/app/src/lib/row-cursor.ts`) tracks the selected row
  as `index`, where `-1` means "no selection".
- `list.down`/`list.up` bindings (`web/packages/app/src/lib/keys/bindings.ts`)
  call `cursor.moveBy(±1)`. From `index === -1`, `moved()` clamps to `0`, so the
  cursor always lands on the first row.
- In the list route (`web/packages/app/src/routes/list.tsx`), clicking a row
  focuses it via `rowCursor.focus(index)`, but `onMouseMove` clears the cursor,
  so after moving the mouse the next `j`/`k` restarts from the first row.

## Desired behavior
- If a row is focused when `j`/`k` is pressed, the first keypress moves relative
  to that focused row (or selects it) rather than jumping to row 0.
- If nothing is focused, keep the current fall-back to the first row.

## Notes
- Add a `tests/` unit test covering: first `j` from a focused row, and first `j`
  with nothing focused (first row).
