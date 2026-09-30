---
status: done
created: 2026-09-28T13:51:51Z
tags:
- feature
- ui
---
# Show a task reference as its key only

A task reference chip shows the status mark, the key, and the title of the task. Change the chip to show the key only.

Apply the change in both places that draw the chip:

- the rendered task body (`task-body.tsx`)
- the live preview of the body editor (`widgets.tsx`)

Both use `TaskRefChip` in `web/packages/task-ui/src/task-ref-chip.tsx`.

## Comments

### 2026-09-30T20:16:12Z by Milan Suk via claude-code

> OPP-140 shipped this work. The chip keeps the status mark in front of the key, as the user decided.
