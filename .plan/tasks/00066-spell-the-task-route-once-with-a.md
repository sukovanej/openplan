---
status: todo
created: 2026-07-31T12:19:40Z
parent: ./00059-web-ui-extract-reusable-componen.md
---
# Spell the task route once with a taskPath helper

The `/task/` route scheme is spelled in nine places across two packages:

- `task-ui`: `parent-link.tsx`, `task-links.ts`, `task-body.tsx` (`TASK_ROUTE`)
- `app`: `routes/list.tsx` (twice), `routes/detail.tsx` (twice), `lib/keys/bindings.ts`, `lib/keys/use-keyboard.ts` (`startsWith("/task/")`), plus the route pattern in `main.tsx`

`task-ui` already links via `react-router-dom` by design, so centralise rather than invert: export `taskPath(id, section?)` from `task-ui` and use it everywhere, the app included. One definition owns the URL shape; the `use-keyboard` prefix check and the `main.tsx` pattern read from the same constant.
