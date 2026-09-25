---
name: task-management-merge
description: Settle a task's status when its work merges. Invoke whenever the user asks to merge a task, its branch, or its PR: "merge OPP-42", "merge this branch", "merge the PR", "land it", "ship it". Decides from its own context whether the merge finishes the task, sets `done` when it is certain, and asks the user when it is not. The repository's own process does the merge.
---

# Merging a task

The task lives in openplan, not in the code. Its status changes with
`openplan set` and never with a commit. Run every step.

Take the task key from the conversation or from the branch name. Work with no
task key has no status to change.

## 1. Decide the status

Answer from your own context. Do not re-read the diff. Does this work finish
everything `OPP-42` asks for?

- Certain it does: set `done` after the merge, in step 3.
- Certain it finishes only part: leave the status, and say which part stays open.
- Anything else, including work you did not write: ask the user, and wait for
  the answer.

Never guess. A wrong `done` closes work that is still open. A request to merge
is the review that `in_review` waits for, so the agent writes `done` here and
nowhere else.

## 2. Merge

Merge the way the repository merges: follow its instructions, its tools, and its
merge method. This skill does not choose them.

## 3. Set the status

When step 1 decided `done`, set it after the merge lands:

```sh
openplan set OPP-42 status done
```

When the merge fails, leave the status.

Report the merge and the task status.
