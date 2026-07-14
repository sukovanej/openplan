---
status: todo
---
# Emit change events for all branches and worktrees

## Goal

Make the daemon detect and broadcast change events for task changes on **any**
branch or worktree, so every connected web UI updates live — not just for writes
the daemon made itself.

Today `/api/events` (SSE) only carries events that the API handlers publish
directly: `create`/`patch`/`delete` emit `TaskChanged { id, branch: "" }` for the
serve-root worktree. The `branch` field is always empty, and changes made in
*other* worktrees or landed on *other* branches produce no event at all. The
file watcher wired in `op-cli/src/serve.rs` is a skeleton (`op-watch`): it watches
only the serve-root `.plan/` and maps every fs modify to a junk
`RefMoved { branch: "" }`.

The read model this pushes to already exists: `aggregate-tasks-across-all-branches`
(done) built the `op-index::Index` matrix, and `GET /api/tasks` is branch-aware
(one aggregated `TaskListItem` per task, with a `BranchState` per branch). This
task is the missing **push** side: watch the real change sources, and publish
correct, per-branch events into the existing `broadcast::Sender<ChangeEvent>`.

## Sources to watch (SPEC §7.5)

1. **Worktree `.plan/` edits** — `notify` on each active worktree's `.plan/tasks/`.
   Catches live working-tree saves (an agent editing a task file in another
   worktree). Drives the "someone is editing this now" (dirty) overlay.
2. **Ref moves** — `refs/heads/*` + `packed-refs`. Commits, merges, rebase
   landings, branch create/delete.
3. **HEAD switches** — `HEAD` + `.git/worktrees/*/HEAD`. A worktree changing
   branches swaps its whole `.plan/`. Required so the dirty overlay stays
   attributed to the branch actually checked out — a switch between two branches
   with identical `.plan/` emits no fs events but still changes which branch is
   "live/dirty", so it is invisible without watching HEAD.
4. **Worktree add/remove** — `.git/worktrees/`. Start watching a new worktree's
   `.plan/`; stop watching a pruned one; the set of live branch columns changes.

## Fine-grained emission (no floods)

A single `git merge` / `rebase` / branch switch touches many refs and files. Do
**not** emit one coarse event per fs notification. Instead, on a ref move or HEAD
switch, diff the affected branch's old vs new `.plan/tasks` tree **by blob OID**
and emit `TaskChanged { id, branch }` only for tasks whose blob actually changed
(added / modified / deleted). Reuse `op-git::Repo::branch_task_blobs` /
`commit_task_blobs` and the `op-index` blob-OID cache, so work is proportional to
*distinct changed versions*, not tasks × branches.

Populate the `branch` field (currently always `""`) with the real branch name so
the client can update the right badge. The `kind` (added/modified/deleted) is
available server-side and already carried by `BranchState`; the client can
refetch the branch-aware list/task to repaint the row, so `TaskChanged { id,
branch }` stays the wire contract for now.

## Debounce & settle

Git operations touch many files rapidly and leave the tree transiently
inconsistent mid-operation. Coalesce bursts of fs events into a single diff pass,
and skip diffing while a git op is in progress — gate on
`op-git::Repo::op_in_progress` / the §7.8 busy-worktree signal, then settle and
diff. (The `op-watch` skeleton's `TODO(skeleton): debounce; settle git ops` is
exactly this.)

## Wiring

- Build the real watcher(s) in `op-watch` (replacing the modify→`RefMoved{""}`
  stub): fs watches for every active worktree's `.plan/`, plus git watches for
  `refs/`, `packed-refs`, `HEAD`, `.git/worktrees/*/HEAD`, and `.git/worktrees/`.
- The daemon already bridges the watcher channel onto the broadcast in
  `op-cli/src/serve.rs` (`Watcher::start(...) → tx → events_tx.send`). Feed the
  diffed `ChangeEvent`s through that existing bridge; no SSE/handler changes
  needed beyond emitting correct events.
- Re-derive the watched worktree set when `.git/worktrees/` changes.

## Out of scope

- **Presence (§7.6)** — the machine-local "who is actively on this task right now"
  channel (`op-presence::Registry`, `PresenceChanged`) is a separate source
  (claim/release + heartbeat reaper, plus a new UI signal). Its own task.
- **Branch-aware single-task read/update/delete** — tracked by
  `branch-aware-crud`.

## Acceptance

With a repo and two worktrees on different branches, and an SSE client watching
from worktree A:

- Editing a task's `.plan` file in worktree B pushes `TaskChanged { branch: B }`.
- A commit / merge on a branch emits per-changed-task events with the correct
  branch — and nothing for tasks whose blob is unchanged.
- Switching worktree B to another branch re-attributes its dirty overlay (the
  amber badge moves), including when the two branches' `.plan/` are identical.
- Adding / removing a worktree starts / stops watching its `.plan/`.
- The web UI's branch badges update live, with no manual refresh.
