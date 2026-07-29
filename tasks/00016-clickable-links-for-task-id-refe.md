---
status: done
created: 2026-07-14T16:10:14Z
---
# Clickable links for [[task-id]] references in markdown

## Goal
Render `[[task-id]]` references in task-body markdown as clickable links to the
referenced task's detail page. Today they show as literal `[[task-id]]` text.

Example: in `9`, the body reads
"Depends on [[./00003-task-crud-across-the-store-daemo.md]] (store + op-api DTOs) and [[./00002-run-the-server-as-a-managed-daem.md]]
(serve root, AppState.index)" — both refs should become links to `/task/<id>`.

## Where
`web/packages/app/src/components/task-body.tsx` renders bodies with
`react-markdown` + `remark-gfm`. The route for a task is `/task/:id` (see `main.tsx`).

## Design
- Add a remark plugin (or a preprocessing pass) that turns `[[<id>]]` tokens into
  markdown/AST links pointing at `/task/<id>`, using react-router navigation
  (client-side, no full page reload) rather than a plain `<a>` that reloads.
- `<id>` is the task filename/id; the `#Section` suffix form (`[[id#Section]]`)
  should link to `/task/<id>` (optionally with the section as a hash/anchor).
- A ref to a non-existent id should degrade gracefully (render as text or a
  visibly-broken link), not crash.
- Style the link consistent with the `prose` theme in light and dark.

## Done when
- `[[...]]` refs in a task body are clickable and navigate to the target task in-app.
- Non-ref bracket text is left untouched.
