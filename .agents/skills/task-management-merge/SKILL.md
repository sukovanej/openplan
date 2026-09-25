---
name: task-management-merge
description: Merge a task's branch or pull request into main. Invoke whenever the user asks to merge a task, its branch, or its PR: "merge OPP-42", "merge this branch", "merge the PR", "land it", "ship it". Decides from its own context whether the merge finishes the task, sets `done` when it is certain and asks the user when it is not, then deletes the branch and the worktree and syncs the local main.
---

# Merging a task

The code branch carries the code only. The task lives in openplan, so its status
changes with `openplan set` and never with a commit. Run every step.

## 1. Decide the status

Answer from your own context. Do not re-read the diff. Does this branch finish
everything `OPP-42` asks for?

- Certain it does: set `done` after the merge, in step 3.
- Certain it finishes only part: leave the status, and say which part stays open.
- Anything else, including a branch you did not write: ask the user, and wait
  for the answer.

Never guess. A wrong `done` closes work that is still open. A request to merge
is the review that `in_review` waits for, so the agent writes `done` here and
nowhere else.

A branch with no task key in its name has no status to change.

## 2. Merge

```sh
gh pr merge <number> --squash --delete-branch
```

`--delete-branch` fails with `'main' is already used by worktree` when the
primary checkout holds main. The merge still happened. Confirm it with
`gh pr view <number> --json state,mergeCommit`, and let step 3 delete the branch.

With no pull request: `git push origin <branch>:main && git push origin --delete <branch>`.

## 3. Set the status and clean up

When step 1 decided `done`, set it now that the merge landed:

```sh
openplan set OPP-42 status done
```

Run these in the primary checkout, never in the worktree you remove:

```sh
git worktree remove .claude/worktrees/<slug>
git branch -D <branch>
git push origin --delete <branch>
git fetch --prune origin
git merge --ff-only origin/main
```

Report the merge commit and the task status.
