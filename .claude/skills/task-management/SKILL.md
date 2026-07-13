---
name: task-management
description: How to manage work items in this repository. Use for any task work — listing, inspecting, creating, updating status/parent/deps, or deleting tasks.
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
```

`--dep` is repeatable; each is a task id (or `task-id#Section`).

## Update

`set <id> <field> <value>` — field is `status`, `parent`, or `deps`.
`deps` is a comma-separated list (empty string clears it).

```sh
oplan set <id> status in_progress
oplan set <id> parent <parent-id>
oplan set <id> deps "<id1>, <id2>"
```

## Delete

```sh
oplan delete <id>          # asks [y/N]
oplan delete <id> --yes    # no prompt
```
