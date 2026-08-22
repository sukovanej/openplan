---
name: task-management-merge
description: Merge a task's branch or pull request into main. Invoke whenever the user asks to merge a task, its branch, or its PR — "merge OPP-42", "merge this branch", "merge the PR", "land it", "ship it". Decides from its own context whether the merge finishes the task, sets `done` in the branch when it is certain and asks the user when it is not, then deletes the branch and the worktree and syncs the local main.
---

# Merging a task

Set the status in the branch, before the merge, so one merge commit carries the
code and the status. Run every step — a merged branch with a live worktree and a
stale local main is not a finished merge.

## 1. Decide the status

You normally did the work, so answer from your own context — do not re-read the
diff to rebuild it. The bar is certainty: does merging this branch finish
everything `OPP-42` asks for?

- Certain it does → set `done`.
- Certain it finishes only part → leave the status, and say which part stays open.
- Anything else, including a branch you did not write → ask the user whether to
  mark the task `done`, and wait for the answer.

Never guess. A wrong `done` closes work that is still open.

A request to merge is the review that `in_review` waits for, so here the agent
writes `done` itself. The `task-management` skill forbids it everywhere else.

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

`--delete-branch` fails with `'main' is already used by worktree` whenever the
primary checkout holds main. The merge still happened — confirm with
`gh pr view <number> --json state,mergeCommit` and let step 3 delete the branch.

With no pull request: `git push origin <branch>:main && git push origin --delete <branch>`.

## 3. Clean up

From the primary checkout, never from the worktree you remove:

```sh
git worktree remove .claude/worktrees/<slug>
git branch -D <branch>
git push origin --delete <branch>
git fetch --prune origin
git merge --ff-only origin/main
```

Report the merge commit and the task status.
