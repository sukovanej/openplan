---
name: task-comments
description: Every task carries an append-only comment log, written with `openplan comment` and read with `openplan comments`. Most tasks get no entry. Invoke this skill only when you must record a fact that the task file, the diff, and the commit message all lose: a departure from the task, work you left out, or a stop before the end. Invoke it also when the user asks to comment on a task or to read a task's comments.
---

# Commenting on a task

The log is append-only, and it is what a reader gets when the chat is gone and
the pull request is closed.

Most tasks get no entry. Write one only when the fact passes both tests:

1. The task file, the diff, and the commit message all lose it.
2. It changes what the next person does.

**Write an entry for:**

- a departure from what the task says, and the reason
- work you left out, or a defect you found and did not fix
- a stop before the end: what is done, what is not
- a decision or a limit that binds a later task, when the code does not show it

**Never for:** progress (the status field), a summary of the diff (the commit),
the specification (edit the body), a question for a user who is here (ask them),
line-level review talk (the pull request), a report that the work is complete or
that the tests pass, a manual check that a test repeats, a doubt you resolved,
your thoughts during the work.

Keep an entry to one or two lines. Write it when the fact appears, on the task
the fact belongs to.

```sh
openplan comments OPP-42                    # read; --json, --all-branches
openplan comment  OPP-42 "One short line."  # append; --body-file - for markdown
```

Run it in the task's worktree. The heading carries the time, the author, and the
agent, so do not sign or date the text. Commit the task file with the work.
