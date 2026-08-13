---
status: done
created: 2026-08-02T13:56:37Z
parent: ./00079-multi-repo-daemon-one-daemon-and.md
dependencies:
- ./00081-serve-n-projects-per-project-wat.md
---
# Merged all-projects board

Serve one board over all projects. `GET /api/board` (unprefixed, daemon-wide)
answers the union of all registered projects. This replaces the
single-project delegation on that route. With one registered project, the
merged board shows exactly what today's board shows. This keeps the embedded
SPA correct until its own rewire lands.

## `project` on the wire

Add a `project` field to `TaskListItem`. Board rows get the field through
`task`. Add the field to `TaskDetail` too. A detail opened from a merged-view
link then knows its coordinate. The per-project routes also fill the field.
One shape serves all routes.

## `Board::build` keys on `(project, id)`

`Board::build` (`op-api/src/lib.rs`) keys `title_of`, `member_ids`,
`children_of`, and `emitted` by the id string alone. Two stores can commit
the same abbreviation, and each then holds an `APP-3`. On a merged board, a
parent reference could then nest a task under a row from a different
project. Key each map in `build` and `emit_row` on `(project, id)`. Resolve a
parent only in the task's own project. Parent references are store-scoped by
construction.

Order in a status group: `board_cmp` first; then the project name and the id
as the tiebreaker. The merged board is then deterministic, and same-project
tasks stay adjacent when ranks tie.

## The merged read

For each `ok` project: rebuild (dirty-gated), then collect
`aggregated_tasks()` with the project name attached. Run one `Board::build`
over the full set. Skip demoted projects. The client learns about them from
`/api/projects`, not from a half-broken board. Take the project index mutexes
one at a time. Never nest them.

## Acceptance

- A test runs over two temporary repositories that share an abbreviation:
  tasks with the same number stay distinct rows, and a child nests only under
  the parent in its own project.
- With one project, the merged board's JSON equals today's board plus the
  additive `project` fields.
- A demoted project drops out of the merged board without an error.
- The OpenAPI spec shows the new field and the new route semantics.
- `cargo build && cargo test && cargo fmt --check && cargo clippy -- -D warnings` pass.
