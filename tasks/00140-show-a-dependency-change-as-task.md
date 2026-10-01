---
status: done
created: 2026-09-27T21:37:32Z
tags:
- feature
- ui
---
# Show a dependency change as task chips in the activity view

A dependency change line in the activity view shows the keys as plain text: `Dependencies: +SIN-9 −SIN-10`. A reader cannot see the status or the title of each task, and cannot click a key.

- Show each added and removed key with `TaskRefChip` (`web/packages/task-ui/src/task-ref-chip.tsx`), the chip that a `[[SIN-9]]` reference in a task body shows.
- Mark each chip with `+` or `−`, as `TagSetChange` in `web/packages/task-ui/src/change-view.tsx` does for tags.
- Give `FieldChangeView` the task refs, so that each chip shows the status and the title. A key that the store cannot resolve shows the dashed chip.
- Keep `fieldChangeText` for the text case. A reorder with no added or removed key still shows `Dependencies reordered`.
- Add a test in `web/packages/task-ui/tests/`.
