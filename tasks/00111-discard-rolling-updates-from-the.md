---
status: done
created: 2026-09-09T20:45:26Z
dependencies:
- ./00110-ui-rolling-updates-control-and.md
tags:
- daemon
- feature
- git
- ui
---
# Discard rolling updates from the review popover

The rolling-updates branch collects task edits until a person publishes them.
Today a person can only publish. An edit that was a mistake stays on the branch
until someone goes to the worktree and undoes it by hand. This task adds discard:
one pending task, or every pending task, from the review popover in
[[./00110-ui-rolling-updates-control-and.md]], on top of the backend in
[[./00109-rolling-updates-a-branch-with-a.md]].

## Git semantics

The daemon worker owns the rolling-updates worktree. A discard runs on that
thread as a new `Signal::Discard`, so it never races a commit or a rebase.

**Discard one task.** In the worktree, restore the task file from the default
branch. When the default branch lacks the file, the task was added, so
`git rm --sparse` it instead. Commit at once with a message such as
"Discard rolling update for OPP-12", disarm the commit timer, and announce
`rolling_updates_changed`. The branch keeps its earlier commits. Its `.plan`
diff against the default branch no longer holds the task, so the cell leaves
`pending` and `rolling_updates_differs` ignores it. Do not rewrite history.

A restore overwrites an uncommitted edit as well, so a dirty cell needs no
separate path. The watcher then fires `edited`, the commit timer runs, and it
finds nothing to commit.

**Discard all.** Run `git rebase --abort` when a rebase is in progress, then
`git reset --hard <default-branch>`, then `ensure_attributes`. Disarm the commit
timer and announce. This is also the way out of the blocked state, which the UI
cannot leave today.

**Blocked.** A discard of one task while a rebase is in progress is refused.
The rebase owns the worktree. Discard all is the only discard that works then.

## API

- `DELETE /api/projects/{project}/rolling-updates` discards everything.
  Answers 204.
- `DELETE /api/projects/{project}/rolling-updates/{task}` discards one task.
  Answers 204. Answers 404 when the task has no pending change. Answers 409
  when a conflict holds the branch.
- Both answer 503 when the repository has no rolling-updates branch, like the
  existing routes.
- Both push `rolling_updates_changed` on the SSE channel, so the control
  refetches with no manual refresh.

Resolve the task file path through the store, the same way a write does. The
`MatrixCell` carries no path.

## UI

In the review popover:

- Each pending row gets a trash icon at its end, visible on hover and on focus.
  A click turns the row tail into an inline confirm: "Discard?" with "Discard"
  and "Keep". No modal.
- Beside "Publish N changes", a quiet "Discard all" button with the same
  two-step confirm.
- In the blocked box, "Discard all" reads "Abort the rebase and discard
  everything" and sits under the two commands.
- Add `useDiscard` next to `usePublish` in `lib/rolling-updates.ts`. Key it on
  `projectMutationsKey`, so a refusal reaches the same error toast. Invalidate
  the rolling-updates query, the project key, and the merged key on settle.
- Show the syncing state while a discard runs.

## Verify

Server tests in `crates/op-server/tests/rolling_updates.rs`:

- Discard one restores the default branch's bytes for a modified task.
- Discard one removes the file of an added task.
- Discard one overwrites a dirty edit that the daemon has not committed.
- Discard one is refused with 409 while a rebase is in progress.
- Discard one answers 404 for a task with no pending change.
- Discard all leaves the branch at the default branch plus the attributes
  commit.
- Discard all clears a blocked rebase and the conflict is gone from the
  rolling-updates route.
- Each discard sends `rolling_updates_changed`.

Web tests in `web/packages/app/tests/rolling-updates.test.ts`:

- The trash icon opens the inline confirm, and "Keep" closes it.
- "Discard" calls the per-task route and the row leaves the list.
- "Discard all" calls the project route.
- The blocked box shows the discard-all action.
