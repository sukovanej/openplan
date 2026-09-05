---
name: task-management
description: Work items in this repo are called "tasks" (.plan/tasks/*.md, managed by the `openplan` CLI), NOT the TODO list or subagent tools. Invoke this skill when the user names a task key (OPP-42), says "the plan", or asks in these words to create a task, list/show/get tasks, work on a task, set a status/parent/dependency, block, reparent, cancel, or delete a task. A request to do work is not a request to track it: do the work, and create no task.
---

# Task management

Tasks are markdown files in `.plan/tasks/`, managed with the `openplan` binary.
A task's key is `OPP-42`, from `abbreviation` in `.plan/config.toml`, and the
CLI takes and prints that spelling only. The key names the file,
`00042-<title-slug>.md`. In a task file, one task names another by its path:
`parent: ./00042-ship-login-page.md`, and `[[./00042-ship-login-page.md]]` in
prose.

Statuses: `backlog` `todo` `in_progress` `in_review` `done` `cancelled`.

## Read

```sh
openplan list                       # id / status / title
openplan list --status in_progress  # filter by status
openplan list --parent <key>        # children of a task
openplan list --json                # [{id,title,status,parent?}]
openplan show <key>                 # metadata: id, title, status, parent, dependencies
openplan get  <key>                 # the raw task file
openplan get  <key> --json          # {id,title,status,parent,dependencies,body}
```

## Create

Create a task only when the user asks for one, in those words. A request to do
work is a request to do the work. Finish it, and leave `.plan/tasks/` untouched.

A new task starts in `backlog`. Pass `--status` only when the user asks for one.

```sh
openplan create "Ship login page"
openplan create "Add validation" --parent <key>
openplan create "Deploy" --status todo --dependency <key> --dependency <key2>
openplan create "Ship login" --body "Support OAuth and email login."
openplan create "Ship login" --body-file notes.md   # or --body-file - for stdin
```

Set the dependencies at creation. A dependency says that the other task must be
complete first, so name only a task that blocks this one. `--parent` groups the
subtasks of a large task.

```sh
openplan create "Add the store schema" --parent OPP-42            # prints OPP-43
openplan create "Read the schema in the API" --parent OPP-42 --dependency OPP-43
```

Tag the task at creation. Read the `Tag` section below.

Do not commit a new task. Leave the file uncommitted until the user approves it.

## Tag

A tag is a registered name. A task can carry a name only after `openplan tag`
registers it. Every store registers `bug`, `feature`, and `draft`, and a project
adds its own names for the areas it splits into.

```sh
openplan tag list                       # name, color, and meaning
openplan create "Fix the parser" --tag bug --tag daemon
openplan set <key> tags "bug, daemon"   # replaces the set; "" clears it
```

Give each task you create one kind (`bug`, `feature`, or `draft`) and each area
name that the work touches. Read `openplan tag list` first and take the names
from what it prints. Never invent a name, and never register one the user did
not ask for. When no name fits, leave the task untagged and say so.

## Update

```sh
openplan set <key> status in_progress
openplan set <key> parent <parent-key>
openplan set <key> dependencies "<key1>, <key2>"   # empty string clears them
openplan set <key> tags "bug, daemon"              # empty string clears them
```

## Work on a task

The user names the task. Do these three steps before you read the code, search
the repository, plan, or start a subagent:

1. `openplan get <key>` reads the task.
2. Create the task's worktree and move into it.
3. `openplan set <key> status in_progress`.

`openplan set` writes to `.plan/tasks/`, so run it in the worktree. Never run it
in the primary checkout, not even to mark a task started before the worktree
exists.

Set `in_review` when the work is complete. A human sets `done`. The one
exception is a merge: follow the `task-management-merge` skill.

## Comment

Follow the `task-comments` skill. Most tasks get no comment.

```sh
openplan comments <key>              # read the log
openplan comment <key> "text"        # append an entry
```

## Delete

```sh
openplan delete <key>          # asks [y/N]
openplan delete <key> --yes    # no prompt
```
