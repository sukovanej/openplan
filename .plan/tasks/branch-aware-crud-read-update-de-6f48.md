---
status: todo
---
# Branch-aware CRUD: read/update/delete a task that lives on multiple branches

Opening a task that exists only on another branch/worktree (e.g. one created
inside a linked worktree) returns "Task not found". CRUD on a task id assumes a
single file in the serve-root checkout, but a task id is really a *set* of
`(branch, blob_oid)` cells across branches/worktrees that may diverge in content
and status. Make read, update, and delete branch-aware; leave create as-is.

## Root cause

- `GET /api/tasks` is branch-aware — it rebuilds `Index` across every branch +
  worktree and returns one aggregated `TaskListItem` per task
  (`op-index/src/lib.rs`), so the task appears in the list.
- `GET /api/tasks/{id}` **without** `?branch=` is not: it does `store.read(&id)`
  against only the serve-root worktree (`op-server/src/lib.rs`, `get_task`). A
  task that isn't a file in that checkout → `NotFound`. The web `getTask` never
  passes a branch (`web/packages/app/src/lib/api.ts`).
- `PATCH`/`DELETE` likewise operate only on the serve-root store, so a
  multi-branch task can't be edited or deleted from the UI either.

## Decisions (resolved)

- **Read:** headline version + branch switcher. Default to the current-worktree
  (headline) version — the same pick `aggregated_tasks()` already makes — and
  surface every branch the task lives on so divergence is visible.
- **Write target (PATCH/DELETE):** checked-out worktrees only. The server
  **never fabricates commits**. A write to a branch that is not checked out in a
  live worktree — or whose worktree is `op_in_progress` — is refused with a clear
  error, not silently retargeted.
- **Delete:** current-worktree-scoped. Delete removes only the one targeted
  worktree's file; the task stays alive on other branches (and will reappear in
  the list). The UI must make clear the delete was local to that branch.
- **Create:** unchanged. Create always mints a new id in the serve root; there is
  no existing multi-branch cell to reconcile.
- **Git required:** the daemon serves a git worktree; there is no valid no-repo
  mode. `oplan serve` refuses to start without a git repository, and all no-repo
  fallback code is removed (see below).
- **Scope:** full read + write in this one task.

## Require a git repository (remove no-repo handling)

Reads now route through the `Index`, which needs a `Repo`, so a repo is a
precondition, not a fallback. Enforce it at startup and delete every degrade path:

- `op-cli/src/serve.rs`: `Repo::discover(root)?` and `Store::discover(root)?`
  (propagate the error) instead of `.ok()` + conditional `with_repo`/`with_store`;
  `oplan serve` fails to start with a clear error when there is no git repo.
- `op-server` `AppState`: make `repo: Repo` and `store: Store` non-optional; drop
  the `with_repo`/`with_store` builders' optionality and construct the state with
  both up front.
- `list_tasks` (op-server, the `if let (Some(repo), Some(store))`): delete the
  `else` branch that lists the store with no branch awareness — always take the
  branch-aware `Index` path.
- `cross_branch_get`: drop the `else { NotFound }` guard for a missing repo/store.
- `require_store` / `StoreError::StoreMissing`: remove the request-time
  "store missing" path now that startup guarantees a store.

## API shape changes

- **Branchless `GET /api/tasks/{id}`** must return the headline effective view
  **plus** the branch set, so a cold-loaded `/task/:id` (no list in memory) can
  render the switcher. Return the headline `TaskView` fields together with a
  `branches: Vec<BranchState>` (as `TaskListItem` already carries), rather than a
  bare `TaskView`. Resolve the body via the `Index` (`effective_view` /
  `effective_raw`) so a task absent from the serve-root checkout still resolves.
- **`?branch=` `GET`** continues to return that branch's effective view (already
  wired via `cross_branch_get` → `effective_view`).
- **`PATCH`/`DELETE`** take a branch/worktree target (`?branch=`, defaulting to
  the current worktree's branch). The server resolves branch → checked-out
  worktree via the same `worktrees()`/`live_worktrees()` logic, refuses if the
  branch is not checked out live or is `op_in_progress`, writes/deletes only that
  worktree's file, and publishes `ChangeEvent::TaskChanged` with **the actual
  branch populated** (today it is always `""`).

## Web

- `getTask` / the detail route consume the new `branches[]` and render a branch
  switcher; switching refetches with `?branch=`.
- Edits/deletes send the branch currently in view.

## Constraints / watch-outs

- Route index rebuilds and file I/O through `spawn_blocking` under flock, as the
  existing branch-aware paths already do.
- Deleting a task on a branch where the file does not exist should 404 honestly
  rather than appear to succeed.

## Acceptance

- Opening a task created in a separate worktree loads its detail view instead of
  "Task not found", with a branch switcher listing every branch it lives on.
- Switching branches in the detail view shows that branch's version.
- Editing status/parent/deps writes to the in-view branch's worktree and is
  refused (clear error) when that branch is not checked out live.
- Deleting removes only the in-view worktree's file; the task survives on other
  branches.
- Running `oplan serve` outside a git repository fails at startup with a clear
  error; there is no degraded no-repo serving mode.
- `cargo build`, `cargo test`, `cargo fmt --check`, `cargo clippy -- -D warnings`
  all pass.
