---
status: done
created: 2026-09-26T01:52:07Z
tags:
- feature
- ui
---
# Show tags as colored chips in the activity view

The activity view and the revision list show tags as plain text. Show each tag with `TagChip`, which uses the color of the tag from the registry. The task page shows tags in the same way.

## Where the view shows tags as text

- A change to the `tags` field of a task. `fieldChangeText` in `task-ui/src/task-change.ts` writes the added and removed names as text. Show each added name and each removed name as a chip. Keep a clear sign for "added" and "removed", for example a `+` or `−` before the chip, or a line through a removed chip.
- A change to a tag in the registry (created, deleted, renamed). `revision-list.tsx` shows the name next to a `Tag` icon, and `TagChangeView` writes a rename as text. Show the name as a chip. For a rename, show the old name and the new name as chips.

## Rules

- Get the color and display name from the tag registry, as the task page does.
- A name that is not in the registry (for example a deleted tag) shows as a dangling chip, the same as `TagChip` with `tag` set to `undefined`.
- The chips must not change the row height of the activity list.
