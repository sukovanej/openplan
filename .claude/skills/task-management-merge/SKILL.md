---
name: task-management-merge
description: Merge a task's branch or pull request into main. Invoke whenever the user asks to merge a task, its branch, or its PR: "merge OPP-42", "merge this branch", "merge the PR", "land it", "ship it". Decides from its own context whether the merge finishes the task, sets `done` in the branch when it is certain and asks the user when it is not, then deletes the branch and the worktree and syncs the local main.
---

# Merging a task

Set the status in the branch, before the merge, so one merge commit carries the
code and the status. Run every step.

## 1. Decide the status

Answer from your own context. Do not re-read the diff. Does this branch finish
everything `OPP-42` asks for?

- Certain it does: set `done`.
- Certain it finishes only part: leave the status, and say which part stays open.
- Anything else, including a branch you did not write: ask the user, and wait
  for the answer.

Never guess. A wrong `done` closes work that is still open. A request to merge
is the review that `in_review` waits for, so the agent writes `done` here and
nowhere else.

```sh
cd <worktree>
openplan set OPP-42 status done
git commit -am "Mark OPP-42 done"
git push
```

A branch with no task key in its name has no status to change.

## 2. Merge

```sh
gh pr merge <number> --squash --delete-branch
```

`--delete-branch` fails with `'main' is already used by worktree` when the
primary checkout holds main. The merge still happened. Confirm it with
`gh pr view <number> --json state,mergeCommit`, and let step 3 delete the branch.

With no pull request: `git push origin <branch>:main && git push origin --delete <branch>`.

## 3. Clean up

Run these in the primary checkout, never in the worktree you remove:

```sh
git worktree remove .claude/worktrees/<slug>
git branch -D <branch>
git push origin --delete <branch>
git fetch --prune origin
git merge --ff-only origin/main
```

Report the merge commit and the task status.
