---
status: done
---
# Full subtask support: unparent, tree/move, ranking, and UI hierarchy

## Goal

The data model already supports arbitrary hierarchy (§2, §3.2): a subtask is a
task with a `parent` pointer, subtasks are their own files, sibling order is a
fractional `rank`. But the surfaced feature is thin — you can set/filter a direct
parent from the CLI and API, and nothing else. Make subtasks *fully* work end to
end: unparenting, tree traversal, reparent + reorder, sibling ranking, and a web
UI that actually renders the hierarchy. Honour "reads global, writes local"
(§7.1) throughout — this adds no cross-branch writes.

## Current state (what's missing)

- **Model (`op-task`)** — complete: `Frontmatter.parent: Option<String>`,
  `set_parent(Option<String>)`. Unknown keys (incl. `rank`) survive
  read-modify-write via `#[serde(flatten)] extra`, but nothing reads/writes
  `rank`.
- **API (`op-api`)** — `parent` rides on `TaskSummary` / `TaskView` /
  `TaskListItem` / `TaskDetail`; `CreateTask.parent` and `TaskPatch.parent`
  exist. Gap: `TaskPatch.parent: Option<String>` is set-only — `None` means
  "leave unchanged", so **there is no way to clear a parent** (unparent to
  top-level). No `rank` on any DTO. No tree/children shape.
- **Server (`op-server`)** — parent flows through the generic
  `create`/`patch`/`get`/`list` routes; no tree or children endpoint.
- **CLI (`op-cli`)** — `create --parent`, `list --parent <id>` (direct children
  only, not recursive), `set <id> parent <value>` (always `Some`, **can't
  clear**), `show` prints `parent`. The specced `oplan tree <id> [--depth N]`
  and `oplan move <id> --parent <id> [--before/--after <id>]` (§8) are **not
  implemented**.
- **Web UI (`web/packages/app`)** — `parent` is decoded into the `TaskListItem`
  and `TaskDetail` schemas (`lib/api.ts`) but **never read, rendered, or
  edited**. The list is flat (no indentation/grouping); the detail page shows no
  parent link, breadcrumb, or child list.

## Design

### 1. Unparent (clear `parent`) — the correctness fix

`set_parent(None)` already exists on the model; expose it up the stack.

- **op-api** `TaskPatch`: distinguish "unset" from "clear". Use a nullable field
  so the JSON `{"parent": null}` clears the parent while an absent key leaves it
  unchanged. Serde-wise, model `parent` as
  `Option<Option<String>>` with `#[serde(default, skip_serializing_if =
  "Option::is_none", deserialize_with = "…double_option")]`, or an explicit
  three-state enum (`Unchanged | Clear | Set(id)`). `apply()`: `Some(None)` →
  `set_parent(None)`, `Some(Some(id))` → `set_parent(Some(id))`, `None` → no-op.
  Same treatment is **not** needed for `deps` (already clears via empty vec).
- **op-cli** `set`: allow `oplan set <id> parent ""` (or a `-`/`none` sentinel)
  to clear the parent, matching how `deps ""` clears deps.

### 2. `tree` — recursive hierarchy read (§8)

`oplan tree <id> [--depth N] [--json]`: print the subtree rooted at `<id>`,
indented, bounded by `--depth` (default unbounded; §8 "cap context"). Traverse
by grouping all tasks on `parent`; order siblings by `rank` (see §4 below), ties
broken by id for stability. `--json` emits a nested
`{id, title, status, children: [...]}` shape. Cycle-safe: a `parent` chain that
loops must not infinite-loop — detect via a visited set and report the offending
id rather than hang. Reads are local-branch-scoped like every other read.

### 3. `move` — reparent + reorder (§8)

`oplan move <id> --parent <id> [--before <id> | --after <id>]`:

- `--parent <id>` reparents (`set_parent`), `--parent ""`/sentinel unparents to
  top level (reuses §1).
- `--before`/`--after <sibling-id>` set `<id>`'s `rank` to fall between the
  neighbouring siblings (see §4). Omitting both appends to the end of the new
  parent's children.
- **Guard against cycles**: reparenting a task under one of its own descendants
  must be refused with a clear error, not allowed to create a cycle.
- Same write target rules as `set`: current worktree/branch only (writes local).

### 4. Sibling `rank` (fractional ordering, §3.2)

- Add `rank` to the model as a first-class, typed frontmatter field
  (`op-task::Frontmatter`) instead of leaving it in `extra`. Keep it
  `skip_serializing_if` empty so a rank-less task's frontmatter stays minimal
  (§3.1: "no parent and no deps → just `status`"). Decide the type: fractional
  index as a string key (lexicographically sortable, e.g. jittered midpoints) is
  preferred over `f64` to avoid precision collapse on repeated `--before`
  inserts — pick during implementation and document the choice in the task.
- Ordering helper: given a parent's ordered children and a
  before/after target, compute the new key that sorts into place. Rebalance only
  when keys collide/exhaust.
- Surface `rank` on the DTOs that need ordering (`TaskSummary`/`TaskListItem`
  for list ordering, `TaskView`/`TaskDetail` for detail). Migration: existing
  task files have no `rank`; treat missing rank as "unranked, sort last by id"
  so nothing breaks before a first `move`.

### 5. Server endpoints

- `GET /api/tasks/{id}/tree?depth=N` — the nested subtree (backs a UI tree view
  and `oplan tree` when talking to the daemon). Built from the branch-aware
  `Index`, same as `list_tasks`.
- Ensure `PATCH /api/tasks/{id}` carries the new clear-parent semantics and the
  optional `rank`. No new write endpoint for `move` is required if `PATCH` can
  express `{parent, rank}` atomically — prefer extending `PATCH` over a bespoke
  route, and have the CLI `move` compute the target `rank` then issue one patch.

### 6. Web UI (§9: hierarchical, drag-to-reorder)

- **List (`routes/list.tsx`)**: render the hierarchy — either indented rows
  (parent → child nesting) or grouped-under-parent, with collapse/expand.
  Siblings ordered by `rank`. A child whose parent is filtered out (e.g. by
  status filter) still needs a sensible placement — decide and note it.
- **Detail (`routes/detail.tsx`)**: show the parent as a breadcrumb / clickable
  link (task ids are already clickable elsewhere) and list direct children with
  their status. Allow changing the parent (including clearing it) and creating a
  child task.
- **Reorder**: `rank` drag-to-reorder is specced (§9) but is the largest UI
  piece — it may split into a follow-up. At minimum this task renders in `rank`
  order and exposes reparent + unparent; call out in "Out of scope" whether
  drag lands here or later.
- Extend `lib/api.ts` schemas with `rank` and add a `getTree`/children accessor;
  thread through `lib/store.ts`.

## CLI surface (this task)

```
oplan set  <id> parent ""                              # unparent (clear) — new
oplan tree <id> [--depth N] [--json]                   # recursive subtree — new
oplan move <id> --parent <id|""> [--before <id> | --after <id>]   # reparent + reorder — new
oplan list --parent <id>                               # unchanged (direct children)
oplan create "<t>" --parent <id>                       # unchanged
```

The actual CLI is flat (`oplan tree`, not `oplan task tree`); match the existing
`create`/`list`/`set`/`show`/`get` command shape in `crates/op-cli/src/main.rs`.

## Acceptance criteria

- [ ] `oplan set <id> parent <p>` then `oplan set <id> parent ""` returns the
      task to top-level (`oplan show <id>` prints `parent: -`); the cleared
      frontmatter drops the `parent` key entirely.
- [ ] `PATCH /api/tasks/{id}` with `{"parent": null}` clears the parent; with the
      key absent, the parent is untouched; with `{"parent": "<id>"}`, it is set.
      Covered by `tower::ServiceExt::oneshot` tests.
- [ ] `oplan tree <root> --json` emits the nested subtree; `--depth 1` bounds it
      to direct children; a `parent` cycle is reported, not hung.
- [ ] `oplan move <id> --parent <new>` reparents; `--before`/`--after` place it
      among siblings in the resulting `oplan tree` / `list` order; reparenting a
      task under its own descendant is refused with a clear error.
- [ ] Siblings created without explicit order, then reordered via `move`, list in
      the new `rank` order; a task with no `rank` sorts last, stably.
- [ ] Web list renders parent→child hierarchy in `rank` order; the detail page
      shows a parent breadcrumb and direct children, and can reparent, unparent,
      and create a child.
- [ ] All writes target the current worktree/branch only; no cross-branch write
      path is introduced.
- [ ] `cargo build`, `cargo test`, `cargo fmt --check`, `cargo clippy -- -D
      warnings` all pass; web unit tests (`vitest`) pass.

## Out of scope (possible follow-ups)

- **Drag-to-reorder in the UI** (§9) if it proves large — this task guarantees
  `rank`-ordered rendering + reparent/unparent; drag can be its own task.
- **Cross-project / multi-store tree views** — hierarchy is within one store.
- **`op task next`** (actionable/unblocked) and `deps`-based `blocked` computation
  — separate concern from hierarchy.
- On-disk `index.db` persistence of tree structure — the index stays rebuildable.

## Notes

- §3.2 makes hierarchy a *reference graph, not physical nesting* — the 200-subtask
  epic stays 200 small files. Every new read (`tree`, children) is built by
  grouping the flat task set on `parent`, reusing the branch-aware `Index`.
- The unparent gap (§1) is a genuine correctness bug in `TaskPatch`/`set`, not
  just a missing feature: today a parent, once set, can never be removed through
  any interface. Land that fix even if the `tree`/`move`/UI pieces split off.
- §11.1 records subtasks-as-files-with-`parent` as **resolved: yes** — this task
  is the delivery of that decision, end to end.

## Implementation notes (as landed)

- **`rank` type**: a base-36 fractional-index string key (`op_task::rank`), not
  `f64`. Digits `0-9a-z` sort lexicographically in ASCII order, so byte
  comparison of keys matches their fractional value, and repeated `--before`
  inserts take midpoints without precision collapse. Missing rank sorts last by
  id (stable). `rank` is a first-class `Frontmatter` field, `skip_serializing_if`
  empty so a rank-less task's frontmatter stays minimal.
- **`move` rank computation** is local (CLI writes files directly): when a
  sibling group's ranks are all valid and strictly increasing it inserts a single
  fractional key between neighbours; otherwise — missing, colliding, malformed,
  or same-point (`a` and `a0`) — it rebalances the whole group onto keys spread
  evenly across the range in one pass (the migration path, and how a group heals
  from hand-edited frontmatter). The server `PATCH` carries `{parent, rank}` for
  single-key moves. The moved task is written before its siblings, so a refused
  move leaves the group untouched.
- **Rank keys are untrusted input**: task files are hand-editable and `PATCH`
  takes a `rank`, so `op-store` rejects a newly written key that is not base-36,
  and `rank::between` returns `None` for a malformed or gapless pair rather than
  descending forever. A rank already on disk never blocks an unrelated edit — the
  rebalance path is what repairs it.
- **Cycle safety**: `op-store` refuses reparenting under a descendant; `tree`
  (CLI + `TaskTree::build`) is cycle-safe via a path-visited set and reports the
  offending id instead of hanging. The web pickers exclude cycle-forming targets
  from a list snapshot that can be stale, so the server's refusal is reachable
  and surfaces in the UI rather than being swallowed.
- **Drag-to-reorder is deferred** to a follow-up (see Out of scope). This task
  ships `rank`-ordered rendering, a parent breadcrumb, a direct-children list,
  and reparent / unparent / create-child controls in the web UI.
