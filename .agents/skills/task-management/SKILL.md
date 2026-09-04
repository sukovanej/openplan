---
name: task-management
description: Work items in this repo are called "tasks" (.plan/tasks/*.md, managed by the `openplan` CLI), NOT the TODO list or subagent tools. Invoke this skill before any task work, and whenever a task is mentioned. Triggers on: create/add/new task, list/show/get task(s), set status/parent/dependencies, add/clear dependency, block, reparent, delete/cancel task, "the plan".
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

Do not commit a new task. Leave the file uncommitted until the user approves it.

## Update

```sh
openplan set <key> status in_progress
openplan set <key> parent <parent-key>
openplan set <key> dependencies "<key1>, <key2>"   # empty string clears them
```

## Work on a task

Do these three steps before you read the code, search the repository, plan, or
start a subagent:

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
