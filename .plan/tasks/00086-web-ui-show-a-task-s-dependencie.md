---
status: backlog
created: 2026-08-25T12:47:10Z
---
# Web UI: show a task's dependencies and dependents on the detail page

The web UI shows a task's parent and its subtasks. It shows nothing about
dependencies. The data is on the wire — `Metadata.dependencies` reaches the
client — but no surface reads it. The only way to see a dependency is
`openplan show`.

Show both directions on the task detail page: the tasks this task waits for,
and the tasks that wait for this one.

## Wire

`TaskDetail` gets two fields:

```rust
pub depends_on: Vec<TaskRef>,
pub blocks: Vec<TaskRef>,
```

`hierarchy_context` builds both. It already holds `by_id` over every aggregated
task, which is all the two passes need. Widen its return; the two `TaskDetail`
sites — the read path in `op-index` and the write path in `op-server` — then
carry the fields with no further change.

`depends_on` resolves each entry of the task's own `dependencies`. It keeps the
order the file gives, because the author wrote that sequence.

`blocks` collects every task whose `dependencies` name this one. It sorts with
`list_item_cmp`, the comparator `children` uses.

A dependency entry may name a section (`OPP-42#Design`). Split on `#` before
each lookup, in both directions. Without the split, a sectioned entry resolves
to no task and renders as a broken reference, and the target never reports that
it blocks anything.

An entry that names no task stays in `depends_on` as an unresolved reference.
`TaskRefChip` already draws one dashed.

## Detail page

Two `Section` blocks below the body, in this order:

1. "Depends on"
2. "Blocks"
3. "Subtasks" (already there)

Each new section is hidden when it is empty. Subtasks stays visible when empty
because it carries the "Add subtask" button; these carry no action.

Each row is a `TaskIdentity` with the task's status, the same as a subtask row.
The status is always the target task's own status. The section does not change
what it says when a dependency completes.

The detail page has one row cursor, `subtaskCursor`. Give it the rows of all
three sections in document order, so `j`, `k`, and `Enter` reach every row on
the page. Rename it `detailCursor`.

## Out of scope

- No graph or canvas view.
- No mark on the list rows.
- No way to set or clear a dependency from the UI.
- No change to `openplan`.
