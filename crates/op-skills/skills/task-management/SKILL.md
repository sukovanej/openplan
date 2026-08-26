---
name: task-management
description: Work items in this repo are called "tasks" (.plan/tasks/*.md, managed by the `openplan` CLI) — NOT the TODO list or subagent tools. Invoke this skill before any task work, and whenever a task is mentioned. Triggers on: create/add/new task, list/show/get task(s), set status/parent/dependencies, add/clear dependency, block, reparent, delete/cancel task, "the plan".
---

# Task management

Tasks in this repo are markdown files in `.plan/tasks/`, managed with the `openplan`
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
openplan list                       # id / status / title
openplan list --status in_progress  # filter by status
openplan list --parent <key>        # children of a task
openplan list --json                # [{id,title,status,parent?}]
openplan show <key>                 # metadata: id, title, status, parent, dependencies
openplan get  <key>                 # the raw task file
openplan get  <key> --json          # {id,title,status,parent,dependencies,body}
```

## Create

Prints the new key. Default status is `backlog` — a new task is not ready to be
picked up until a human says so. Pass `--status` only when the user asks for one.

```sh
openplan create "Ship login page"
openplan create "Add validation" --parent <key>
openplan create "Deploy" --status todo --dependency <key> --dependency <key2>
openplan create "Ship login" --body "Support OAuth and email login."
openplan create "Ship login" --body-file notes.md   # or --body-file - for stdin
```

`--dependency` is repeatable; each is a task key (or `<key>#Section`).

Set the dependencies when you create a task. A dependency shows that another
task must be complete first, so the file records the order of the work.

Decomposition is the usual case. `--parent` groups the subtasks of a large task,
and `--dependency` puts them in order. When a subtask needs the result of a
sibling, name that sibling.

```sh
openplan create "Add the store schema" --parent OPP-42            # prints OPP-43
openplan create "Read the schema in the API" --parent OPP-42 --dependency OPP-43
```

Name only a task that must be complete first. When a person can do two subtasks
in any order, do not put a dependency between them.

`--body` / `--body-file` set the markdown content below the title heading; they
are mutually exclusive.

When asked to create a task, do the write in a worktree as usual, but do **not**
commit it right away. Leave the change uncommitted and wait for the user to
review the new task first; commit (and merge) only after they approve.

## Update

`set <key> <field> <value>` — field is `status`, `parent`, or `dependencies`.
`dependencies` is a comma-separated list (empty string clears it).

```sh
openplan set <key> status in_progress
openplan set <key> parent <parent-key>
openplan set <key> dependencies "<key1>, <key2>"
```

## Working on a task

`openplan set <key> status in_progress` writes to `.plan/tasks/`, so it is a
tracked-file change subject to the same worktree discipline as any other write.
Start each task in this order, and do these three steps before anything else:

1. `openplan get <key>` — read the task.
2. Create the task's worktree and move into it.
3. `openplan set <key> status in_progress` — immediately, as the next command.

```sh
# from the task's worktree, not the primary checkout:
openplan set <key> status in_progress
```

Mark the task `in_progress` before you read the code, search the repository,
plan, or start a subagent. The status tells the user that you took the task, so
it must be true from the first minute. Exploration is not a reason to wait: it
takes minutes, and for all of them the board shows the task as free.

Never run `openplan set` from the primary checkout to "just mark it started" before
the worktree exists — that is the write the Worktree rule forbids.

When the work is complete, mark it `in_review` — not `done`:

```sh
openplan set <key> status in_review
```

`in_review` means the work is finished and awaiting human review. Do not set a
task to `done` yourself; a human moves it from `in_review` to `done` (or back to
`in_progress` if the review asks for changes).

The one exception is a merge: when the user asks to merge the task's branch,
follow the `task-management-merge` skill.

## Comments

Every task carries an append-only comment log. Follow the `task-comments` skill
when you depart from what the task says, decide something it leaves open, keep a
doubt, or stop before the end.

```sh
openplan comments <key>              # read the log
openplan comment <key> "text"        # append an entry
```

## Delete

```sh
openplan delete <key>          # asks [y/N]
openplan delete <key> --yes    # no prompt
```
