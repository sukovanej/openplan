---
name: task-management
description: Work items in this repo are called "tasks" (.plan/tasks/*.md, managed by the `oplan` CLI) — NOT the TODO list or subagent tools. Invoke this skill before any task work, and whenever a task is mentioned. Triggers on: create/add/new task, list/show/get task(s), set status/parent/deps, add/clear dependency, block, reparent, delete/cancel task, "the plan".
---

# Task management

Tasks in this repo are markdown files in `.plan/tasks/<id>.md`, managed with the
`oplan` binary.

Statuses: `backlog` `todo` `in_progress` `done` `cancelled`.

## Read

```sh
oplan list                       # id / status / title
oplan list --status in_progress  # filter by status
oplan list --parent <id>         # children of a task
oplan list --json                # [{id,title,status,parent?}]
oplan show <id>                   # metadata: id, title, status, parent, deps
oplan get  <id>                   # the raw task file
oplan get  <id> --json           # {id,title,status,parent,deps,body}
```

## Create

Prints the new id. Default status is `todo`.

```sh
oplan create "Ship login page"
oplan create "Add validation" --parent <id>
oplan create "Deploy" --status backlog --dep <id> --dep <id2>
oplan create "Ship login" --body "Support OAuth and email login."
oplan create "Ship login" --body-file notes.md   # or --body-file - for stdin
```

`--dep` is repeatable; each is a task id (or `task-id#Section`).

`--body` / `--body-file` set the markdown content below the title heading; they
are mutually exclusive.

## Update

`set <id> <field> <value>` — field is `status`, `parent`, or `deps`.
`deps` is a comma-separated list (empty string clears it).

```sh
oplan set <id> status in_progress
oplan set <id> parent <parent-id>
oplan set <id> deps "<id1>, <id2>"
```

## Working on a task

Before starting work on a task, mark it `in_progress`:

```sh
oplan set <id> status in_progress
```

Do this as the first step, before making any changes. Mark it `done` once the
work is complete.

## Delete

```sh
oplan delete <id>          # asks [y/N]
oplan delete <id> --yes    # no prompt
```
