---
name: task-comments
description: Every task carries an append-only comment log, written with `openplan comment` and read with `openplan comments`. Invoke while you work on a task — when you depart from what the task says, decide something it leaves open, keep a doubt, leave work out, hit a constraint, stop before the end, or verify something no test holds. Invoke it also when the user asks to comment on a task or to read a task's comments.
---

# Commenting on a task

The log is what a reader gets when the chat is gone and the pull request is
closed. It is append-only: you never take an entry back.

Comment when a person who reads only the task file loses something. When the
body, the diff, or the commit message answers it, write nothing.

**Write an entry for:**

- a departure from what the task says, and the reason
- a choice the task left open
- a doubt you did not act on
- work you left out, or a defect you found and did not fix
- a constraint that shapes the next change
- a stop before the end: what is done, what is not
- a check that no test holds

**Never for:** progress (the status field), a summary of the diff (the commit and
the pull request), the specification (edit the body), a question for a user who
is here (ask them), line-level review talk (the pull request).

Write at the moment the fact appears. Comment on the task the fact belongs to,
not the task you have open. Set `in_review` with no entry when nothing applies.

```sh
openplan comments OPP-42                    # read; --json, --all-branches
openplan comment  OPP-42 "One short line."  # append; --body-file - for markdown
```

Run it in the task's worktree. The heading carries the time, the author, and the
agent, so do not sign or date the text. Commit the task file with the work.
