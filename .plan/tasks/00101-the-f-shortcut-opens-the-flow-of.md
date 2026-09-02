---
status: in_review
created: 2026-09-02T09:36:42Z
---
# The f shortcut opens the flow of the task at hand

The `f` key shows the flow of one task. Today it works on the task detail only.

Make `f` work wherever a task is at hand: on a list, when a row is selected or under the pointer, `f` opens the flow page for that task. Use the target the copy-id shortcut already uses — the row under the pointer, then the selected row, then the task of the route.

## Comments

### 2026-09-02T09:40:44Z by Milan Suk via claude-code

> The f binding moved from the detail scope to the global scope, and now shares the target of the copy-id shortcut: the row under the pointer, then the selected row, then the task of the route. It therefore works on a list, and on a detail it can also show the flow of a selected subtask. The detail page keeps its Flow link, but the keyboard no longer goes through the detail-action bus. copy-target.ts became row-target.ts, because two shortcuts read the target now. Verified in a browser: list selection, hover, and the detail route each open the correct flow.
