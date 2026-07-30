---
name: task-management
description: Work items in this repo are called "tasks" (.plan/tasks/*.md, managed by the `oplan` CLI) — NOT the TODO list or subagent tools. Invoke this skill before any task work, and whenever a task is mentioned. Triggers on: create/add/new task, list/show/get task(s), set status/parent/dependencies, add/clear dependency, block, reparent, delete/cancel task, "the plan".
---

# Task management

Tasks in this repo are markdown files in `.plan/tasks/`, managed with the `oplan`
binary. A task's id is its key — `OPP-42`, from `abbreviation` in
`.plan/config.toml` — and that is the only spelling the CLI takes or prints. A
bare `42`, a lowercased `opp-42`, and a padded `OPP-042` are refused. The number
behind the key names the file, `00042-<title-slug>.md`, where only the leading
digits identify it.

Inside a task file, one task names another by its file — `parent:
./00042-ship-login-page.md`, and `[[./00042-ship-login-page.md]]` in prose — so
the directory reads on its own. The CLI takes and prints the key; the store
writes the path.

Statuses: `backlog` `todo` `in_progress` `in_review` `done` `cancelled`.

## Read

```sh
oplan list                       # id / status / title
oplan list --status in_progress  # filter by status
oplan list --parent <key>        # children of a task
oplan list --json                # [{id,title,status,parent?}]
oplan show <key>                 # metadata: id, title, status, parent, dependencies
oplan get  <key>                 # the raw task file
oplan get  <key> --json          # {id,title,status,parent,dependencies,body}
```

## Create

Prints the new key. Default status is `backlog` — a new task is not ready to be
picked up until a human says so. Pass `--status` only when the user asks for one.

```sh
oplan create "Ship login page"
oplan create "Add validation" --parent <key>
oplan create "Deploy" --status todo --dependency <key> --dependency <key2>
oplan create "Ship login" --body "Support OAuth and email login."
oplan create "Ship login" --body-file notes.md   # or --body-file - for stdin
```

`--dependency` is repeatable; each is a task key (or `<key>#Section`).

`--body` / `--body-file` set the markdown content below the title heading; they
are mutually exclusive.

When asked to create a task, do the write in a worktree as usual, but do **not**
commit it right away. Leave the change uncommitted and wait for the user to
review the new task first; commit (and merge) only after they approve.

## Update

`set <key> <field> <value>` — field is `status`, `parent`, or `dependencies`.
`dependencies` is a comma-separated list (empty string clears it).

```sh
oplan set <key> status in_progress
oplan set <key> parent <parent-key>
oplan set <key> dependencies "<key1>, <key2>"
```

## Working on a task

`oplan set <key> status in_progress` writes to `.plan/tasks/`, so it is a
tracked-file change subject to the same worktree discipline as any other write.
The correct order is: create/switch into the task's worktree **first**, then make
marking it `in_progress` the first write you do *inside* that worktree.

```sh
# from the task's worktree, not the primary checkout:
oplan set <key> status in_progress
```

Never run `oplan set` from the primary checkout to "just mark it started" before
the worktree exists — that is the write the Worktree rule forbids.

When the work is complete, mark it `in_review` — not `done`:

```sh
oplan set <key> status in_review
```

`in_review` means the work is finished and awaiting human review. Do not set a
task to `done` yourself; a human moves it from `in_review` to `done` (or back to
`in_progress` if the review asks for changes).

## Delete

```sh
oplan delete <key>          # asks [y/N]
oplan delete <key> --yes    # no prompt
```
