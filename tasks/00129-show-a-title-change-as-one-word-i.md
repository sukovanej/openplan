---
status: todo
created: 2026-09-26T13:43:55Z
tags:
- feature
- ui
---
# Show a title change as one word in the activity view

A title change line in the activity view shows the old title and the new title: `Title: "Old" → "New"`. Long titles make the line long, and it wraps. The diff popover (OPP-124) shows the two titles, so the line does not need them.

- Show a title change as `Title` with the `Type` icon. A description change shows `Description` in the same way.
- `fieldChangeText` in `web/packages/task-ui/src/task-change.ts` makes the text. The activity view, the task history, and the revision notice of a task snapshot all use it, so all three get the change.
- Remove the `quoted` helper when no code uses it.
- Update the title case in `web/packages/task-ui/tests/task-change.test.ts`.
