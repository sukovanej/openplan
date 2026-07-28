---
status: in_review
created: 2026-07-16T18:50:25Z
---
# Daemon as single write/id authority: route CLI CRUD through the HTTP API

Make the daemon the single authority for task write/CRUD operations so id
allocation happens in exactly one place. Route `oplan` CLI mutations through the
daemon's HTTP API (the same `POST /api/tasks`, `PATCH /api/tasks/{id}`,
`DELETE /api/tasks/{id}` the web UI already uses) instead of calling `op-store`
directly. This is the allocation mechanism that unblocks
[[task-identity-incrementing-integ-a4a7]] (incrementing integer ids): a single
writer with a view across all local branches can hand out monotonic ids.

## Today — most of the machinery already exists

- CLI `create/set/delete` call `Store::discover(root).create(...)` directly
  (`op-cli/src/main.rs`). `op-client` only exposes `health`/`shutdown`.
- The daemon **already writes into arbitrary worktrees** for `patch`/`delete`:
  `write_branch()` picks the target branch (`?branch=`, else the serve-root's
  branch), `index.live_store(&branch)` resolves it to the worktree that has that
  branch checked out and writes there via `op-store`'s flock, and
  `ApiError::NotWritable` refuses when no live worktree has the branch ("the
  server never fabricates a commit").
- **`create_task` is the laggard**: it ignores `?branch=` and calls
  `state.store.create(...)` on the serve-root store, with no global allocation.

So this task is narrow: make `create` symmetric with `patch`/`delete`, make id
allocation global, and route the CLI through the HTTP API.

## Design decisions (settled — single-machine scope)

open-plan is **single-machine**. That anchors the design:

1. **Per-machine authority is enough.** Cross-machine / multi-clone uniqueness is
   an explicit **non-goal**. One machine daemon is the sole in-band writer, so ids
   are globally unique across all local branches/worktrees.
2. **Write target is a *branch*, not a worktree path — this is what keeps writes
   local.** A write resolves to the live worktree of its branch at write time
   (`live_store`), or is refused (`NotWritable`). A branch switch between the CLI
   call (which pins `?branch=`) and the daemon write therefore yields either a
   correct-branch/other-worktree write or a refusal — **never a wrong-branch
   write.** `create` must adopt this same `write_branch` + `live_store` path;
   creating a task consequently requires a live worktree for its branch (same rule
   as edits — document it).
3. **Global-across-branch allocation ⇒ no merge repair.** The sole writer derives
   its floor across *all* local branches (the set the §7.4 matrix builder already
   reads), so any integer is issued exactly once. Two *different* task-`N`s can
   never exist; a merge only ever sees one task `N` with divergent versions.
   `mergedriver.rs` stays a pure content merger — no renumber / ref-rewrite pass.
4. **Allocation must be atomic in-process, not a per-request `max+1`.** The
   handlers deliberately release the index mutex *before* the flock write (to keep
   reads unblocked), so two concurrent creates that each compute `max(matrix)+1`
   would both pick the same number. Allocation therefore lives in an **in-memory
   monotonic counter in `AppState`** (seeded from the matrix max at startup /
   whenever the floor advances), bumped atomically at allocation. No committed
   counter file (it would conflict on every parallel-branch merge and re-diverge
   per branch) and nothing durable in `index.db` (a rebuildable content-addressed
   cache). Self-healing across restarts: reseed from the matrix.

## Change

- Extend `op-client` with CRUD methods; make CLI `create/set/delete` call them,
  passing the caller's branch as `?branch=`.
- Route through the machine daemon; auto-start it if down (see daemon-lifecycle).
- `create_task`: adopt `write_branch` + `live_store` (like `patch`/`delete`) and
  allocate the id from the in-memory counter (#4) instead of `store.create`'s
  random suffix.

With the daemon as sole in-band writer there is no in-band collision source. A
person can still hand-create a file that bypasses the daemon and duplicates an id
— but that exposure exists identically with today's random suffix, so warning on
it is **out of scope** here. (If wanted later, "warn on duplicate task ids" is a
standalone task that would apply to the hex scheme too.)

## Residual risks (pre-existing for patch/delete; low on single-user)

- A `git checkout`/`merge`/`reset` that swaps the target worktree's whole tree
  *during* the daemon's flock write — git is not flock-aware, so there is a window.
- Foreign-worktree file ownership: daemon-user-owned files in a worktree the human
  edits. Mostly moot for a single user.

## Acceptance

- CLI `create/set/delete` go through the daemon HTTP API; behavior matches the UI.
- `create` resolves its target via `write_branch`/`live_store` like `patch`/`delete`.
- Exactly one in-memory allocator issues ids; concurrent creates never collide.
- No-daemon behavior is defined: CLI auto-starts it; failure mode is explicit.
- `cargo build && cargo test && cargo fmt --check && cargo clippy -- -D warnings` pass.
