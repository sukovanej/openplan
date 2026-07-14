---
status: done
---
# Aggregate tasks across all branches and worktrees (task×branch matrix)

## Goal
Make a task visible on every branch it exists on, from any worktree — the read side of §7.
Today `op-index::Index` holds an empty `Matrix`, `op-git::Repo` can only list branch names, and
`GET /api/matrix` returns nothing. Build the live **task×branch matrix** from the object DB and
active worktrees, and expose cross-branch **reads** through the CLI and daemon. Honour the core
invariant (§7.1): **reads are global, writes stay local** — this task adds no cross-branch writes.

## Design

### Reads global, writes local (§7.1)
One logical task (id = filename) has one **version per branch**. This task reads all of them; it
never mutates a task on a branch other than the checked-out worktree's. Mutating another branch's
version stays impossible — `op-store` writes still target the current worktree only.

### Building the matrix from the object DB (§7.4)
For each local branch (`refs/heads/*`, already reachable via `Repo::local_branches`), walk that
branch's `.plan/tasks/` tree through the object DB (gitoxide) — **no per-file `git show` shell-outs**:
- Emit one `(task-id, branch, blob_oid)` triple per task file found in the tree.
- **Dedup by blob OID.** A task file unchanged across branches is one blob shared by many branches;
  parse each *distinct* blob exactly once. Work is proportional to distinct versions, not
  tasks × branches.
- **Blob-OID cache** keyed by OID → parsed `TaskSummary` (the existing `Index::blob_cache`,
  content-addressed so it never invalidates). In-memory for this task; on-disk `index.db`
  persistence (§4) is a follow-up.

### Effective state overlays the working tree (§7.4)
The effective state of `(task, branch)` is the **dirty working-tree copy** when some worktree has
that branch checked out, else the **committed blob** at the branch HEAD:
- Enumerate linked worktrees (gitoxide worktree API — the `git worktree list --porcelain`
  equivalent): each yields a path + its checked-out branch.
- For a checked-out branch, read the task files from that worktree's on-disk `.plan/tasks/` and mark
  the cell `dirty` when the working file's content differs from the HEAD blob OID (hash the working
  bytes, compare OIDs). Uncommitted/new-on-disk tasks appear as dirty cells; committed-only tasks
  use the blob.
- Skip a worktree while a git op is mid-flight (`Repo::op_in_progress` already detects
  `MERGE_HEAD` / `rebase-*`) so the matrix never shows torn state (§7.8); it refreshes once settled.

### The matrix as data (§7.9 reads)
`Index` populates `Matrix { cells: Vec<MatrixCell{ branch, task: TaskSummary, blob_oid, dirty }> }`
(types already in `op-api`). Add read-side helpers on `Index`:
- `rebuild(&Repo, &Store)` — full recompute from all branches + worktrees (dedup + cache).
- per-task view: given an id, the branches it lives on grouped by identical blob OID ("these
  branches match, these diverge") — this backs `show --branches` and the UI's per-task badges.
- cross-branch read: `(id, branch)` → the effective `Task`/`TaskView` (working copy or blob).

### Conflict markers don't break parsing (§7.7)
A blob may carry unresolved `<<<<<<<` markers from a merge. Parsing a cell's `TaskSummary` must not
panic on them — fall back to a minimal summary (id + best-effort title, status unknown) and flag the
cell rather than aborting the whole rebuild. The full conflicted-task UX and the merge driver are
out of scope (their own task).

## CLI surface (this task)
```
oplan list  --all-branches [--json]        # every task on every local branch (matrix rows)
oplan list  --branch <name> [--json]       # tasks as they stand on one branch (read-only if not checked out)
oplan show  <id> --branches                # per-branch status matrix for one task (grouped by version)
oplan get   <id> --branch <name> [--json]  # read one task's version on another branch (read-only)
```
- Default scope is unchanged: no flag = the current worktree/branch, resolved locally (§7.9).
- `--all-branches` / `--branch` / `--branches` are **reads**; there is deliberately no
  cross-branch write flag. `set`/`delete` keep targeting the current worktree only.
- Extends the existing `list`/`show`/`get` in `crates/op-cli/src/main.rs`; `branches` stays as-is.

## Daemon / HTTP (this task)
- `GET /api/matrix` (route already wired) returns the **populated** matrix instead of an empty one;
  `AppState` already carries `Arc<Mutex<Index>>`. Build/refresh the index from the serve root's repo
  on startup and on demand.
- `GET /api/tasks/:id?branch=<name>` — cross-branch read of one task's version (branch omitted =
  current, the existing behaviour). Reuse `TaskView`.
- Refresh trigger for v1 is coarse (rebuild on request / on the existing watcher's change signal).
  Fine-grained incremental invalidation is out of scope — see below.

## Crate changes
- **op-git**: add object-DB reads on `Repo`: list a branch's `.plan/tasks/` blobs
  (`Vec<(task_id, blob_oid)>`) and fetch blob bytes by OID; enumerate linked worktrees
  (path + checked-out branch); a helper to hash working-file bytes to a blob OID for the dirty check.
  Keep `local_branches` / `op_in_progress`.
- **op-index**: real `rebuild(&Repo, &Store)` populating `matrix` + `blob_cache` (dedup by OID);
  the per-task grouped view and `(id, branch)` effective-read helpers; keep `matrix()` /
  `cached_versions()`.
- **op-api**: reuse `Matrix` / `MatrixCell` as-is where possible. Add only what the grouped
  per-task view genuinely needs (e.g. a `TaskBranches` view) rather than reshaping existing DTOs.
- **op-server**: `matrix` handler returns the built matrix; add the `?branch=` query on the task
  GET; wire index rebuild into serve startup.
- **op-cli**: `--all-branches` / `--branch` on `list`, `--branches` on `show`, `--branch` on `get`;
  resolve against `op-index` over the discovered `Repo`. Reads work headless (daemon down) by
  building the matrix directly, matching task-crud's local-first read stance.

## Acceptance criteria
- [ ] With a task committed on two branches, `oplan list --all-branches --json` emits one matrix row
      per (task, branch); a file identical across branches parses **once** (assert via
      `Index::cached_versions` < rows).
- [ ] `oplan list --branch <other>` shows that branch's versions without checking it out and writes
      nothing; `--branch` on a non-existent branch errors non-zero with a clear message.
- [ ] `oplan show <id> --branches` lists each branch's status for the task, grouping branches that
      share a blob OID and flagging divergent ones.
- [ ] `oplan get <id> --branch <name>` prints that branch's version; for a branch checked out in
      another worktree with uncommitted edits, it reflects the **working-tree** copy (dirty), not the
      HEAD blob.
- [ ] A branch whose `.plan/tasks/<id>.md` contains `<<<<<<<` conflict markers yields a flagged cell,
      not a panic or an aborted rebuild.
- [ ] `GET /api/matrix` returns the populated matrix; `GET /api/tasks/:id?branch=<name>` round-trips a
      cross-branch read (missing id/branch → 404). Covered by `tower::ServiceExt::oneshot` tests.
- [ ] No cross-branch write path exists: `set`/`delete`/PATCH still target the current worktree only.
- [ ] `op-index` tests over a fixture repo (multiple branches + a linked worktree, built with
      `tempfile` + gitoxide): matrix shape, blob-OID dedup, and working-tree-overlay dirty flag.
- [ ] `cargo build`, `cargo test`, `cargo fmt --check`, `cargo clippy -- -D warnings` all pass.

## Out of scope (follow-up tasks)
- **Realtime incremental refresh** — `notify` watchers, `HEAD`/`refs/` watching, and `diff-tree
  old..new` to re-parse only changed blobs on a ref move (§7.4 incremental, §7.5). Lands with the
  **op-watch** task; here the matrix rebuilds coarsely.
- **Presence / coordination** — `claim` / `release` / `claim-status`, heartbeat expiry, live presence
  dots (§7.6), and busy-worktree write gating (§7.8). The **op-presence** task.
- **Section-aware merge driver** and the conflicted-task resolution UX (§7.7). Its own task; here we
  only avoid choking on markers.
- **On-disk `index.db`** persistence of the blob-OID cache (§4) and remote-tracking refs (§7.10).
- The web UI consuming the matrix (branch badges, version grouping, swimlanes) — §9 UI tasks.

## Notes
- Depends on [[task-crud-6e8b]] (store + `op-api` DTOs) and [[daemon-lifecycle-45b1]] (serve root,
  `AppState.index`); both are done.
- "Reads global, writes local" (§7.1) is the invariant that keeps branch data branch-scoped — the
  test asserting no cross-branch write path is guarding the whole design, not a formality.
- Prefer gitoxide object-DB/tree walks over shelling out to `git`; dedup by blob OID is what keeps a
  200-task × 20-branch repo cheap (§7.4).
</content>
</invoke>
