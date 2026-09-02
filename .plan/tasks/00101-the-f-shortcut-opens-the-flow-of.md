---
status: done
created: 2026-09-02T09:36:42Z
---
# The f shortcut opens the flow of the task at hand

The `f` key shows the flow of one task. Today it works on the task detail only.

Make `f` work wherever a task is at hand: on a list, when a row is selected or under the pointer, `f` opens the flow page for that task. Use the target the copy-id shortcut already uses — the row under the pointer, then the selected row, then the task of the route.

## Comments

### 2026-09-02T09:40:44Z by Milan Suk via claude-code

> The f binding moved from the detail scope to the global scope, and now shares the target of the copy-id shortcut: the row under the pointer, then the selected row, then the task of the route. It therefore works on a list, and on a detail it can also show the flow of a selected subtask. The detail page keeps its Flow link, but the keyboard no longer goes through the detail-action bus. copy-target.ts became row-target.ts, because two shortcuts read the target now. Verified in a browser: list selection, hover, and the detail route each open the correct flow.

### 2026-09-02T10:05:02Z by Milan Suk via claude-code

> Review fixes: a key named in a flow query now seeds the flow whatever the status of the task, so f on a done or cancelled row no longer shows an empty page. An explicit status still narrows a named key. The target logic moved into one taskAtHand function in row-target.ts, and liveCursor in row-cursor.ts holds the choice between the list cursor and the detail cursor, so the test harness and the app share both. Two findings stay open for a separate task: a route that renders a skeleton leaves the cursor stale, and an unresolved dependency row can send f to a task that does not exist.
