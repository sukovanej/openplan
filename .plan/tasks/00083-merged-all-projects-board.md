---
status: backlog
created: 2026-08-02T13:56:37Z
parent: ./00079-multi-repo-daemon-one-daemon-and.md
dependencies:
- ./00081-serve-n-projects-per-project-wat.md
---
# Merged all-projects board

One board over every project: `GET /api/board` (unprefixed, daemon-wide)
answers the union of all registered projects, replacing the single-project
delegation that route carries today. With one project registered the merged
board renders exactly what today's board does — which is what keeps the
embedded SPA working until its own rewire lands.

## `project` on the wire

`TaskListItem` gains a `project` field (board rows inherit it via `task`), and
`TaskDetail` carries it too so a detail fetched from a merged-view link knows
its coordinate. Per-project routes fill it as well — one shape everywhere.

## `Board::build` keys on `(project, id)`

`Board::build` (`op-api/src/lib.rs`) keys `title_of`, `member_ids`,
`children_of`, and `emitted` by the id string alone. Two projects whose stores
committed the same abbreviation both hold an `APP-3`; under a merged board a
parent reference would then nest a task under another project's row. Every map
in `build` and `emit_row` keys on `(project, id)`, and parent resolution only
ever looks inside the task's own project — parent references are store-scoped
by construction.

Ordering inside a status group: `board_cmp` first, project name then id as the
tiebreaker, so the merged board is deterministic and same-project tasks read
contiguously where ranks tie.

## The merged read

For each `ok` project: rebuild (dirty-gated), collect `aggregated_tasks()`
tagged with the project name; one `Board::build` over the concatenation.
Demoted projects are skipped here — the client learns about them from
`/api/projects`, not from a half-broken board. Project index mutexes are taken
one at a time, never nested.

## Acceptance

- A test over two temp repositories sharing an abbreviation: same-numbered
  tasks stay distinct rows, and a child nests only under its own project's
  parent.
- With one project, the merged board's JSON equals today's board plus the
  additive `project` fields.
- A demoted project drops out of the merged board without erroring it.
- OpenAPI spec reflects the new field and route semantics.
- `cargo build && cargo test && cargo fmt --check && cargo clippy -- -D warnings` pass.
