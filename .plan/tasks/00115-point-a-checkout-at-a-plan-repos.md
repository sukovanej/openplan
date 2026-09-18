---
status: backlog
created: 2026-09-18T09:28:32Z
tags:
- cli
- feature
---
# Point a checkout at a plan repository with .openplan.toml

## Problem

A company repository often cannot hold the plan. The company forbids a new
tracked directory, or the team does not want task files in its history. Today
the only answer is a plan repository of its own, plus `--root ~/plans/acme` on
every command. An agent or a script that runs with the working directory inside
the company checkout gets no help from that flag, because the agent skills call
bare `openplan`.

## Decision

A tracked pointer file in the company repository names the plan repository. The
CLI resolves its root from that file. Git, the store, the branches, and the
project identity all come from the plan repository. Nothing about the data model
changes.

```toml
# .openplan.toml, in the company repository root
plan_root = "../acme-plan"
```

## Root resolution

The CLI resolves the root one time, before it dispatches the command.

```d2
direction: down
start: "resolve the root"
flag: "--root given?"
file: ".openplan.toml found\nabove the working directory?"
key: "it holds plan_root?"
use_flag: "use the flag path" {style.fill: "#dff"}
use_pointer: "use the expanded plan_root" {style.fill: "#dff"}
use_cwd: "use the working directory" {style.fill: "#dff"}
start -> flag
flag -> use_flag: yes
flag -> file: no
file -> key: yes
file -> use_cwd: no
key -> use_pointer: yes
key -> use_cwd: no
```

Rules for `plan_root`:

- A relative path resolves against the directory of the pointer file.
- A leading `~` expands to the home directory. `${VAR}` expands from the
  environment.
- An absolute path is an error. The file is tracked, so one developer's layout
  must not enter company history. The error tells the user to write a relative
  path or a `~` path.
- A missing target directory is an error that names the path and the pointer
  file.
- A pointer file with no `plan_root` key is not an error. The search continues
  upwards, then the working directory wins.
- A pointer that resolves to a directory with no `.plan` store gives the
  store's own error, not a new one.

The pointer never applies to `setup-skills`. Skills belong to the checkout the
person writes code in, so that command keeps the literal root.

## Branch

The plan repository's own checked-out branch is the target of every read and
write. The company branch name does not map across. This keeps the behaviour of
`--root` today, and it keeps one working tree with one current branch.

## Not in scope

A store outside the git repository that keeps the company repository as the
source of branches and history. Per-branch reads come from committed blobs in
the same repository: `Index::rebuild` walks `local_branches()` and reads
`.plan/tasks` from each branch tree. A store that never enters the trees makes
every cell dirty, so `updated` falls back to the file mtime, the branch columns
all show the same tasks, a second worktree shows none, and the merge driver has
no paths to act on. That is a second storage backend, not a configuration key.

## Verify

- `crates/op-cli/tests/`: a company directory with `.openplan.toml` that points
  at a sibling plan repository. `openplan create` in the company directory
  prints a key from the plan repository's abbreviation, and the task file appears
  under the plan repository. `openplan list` in the company directory prints it.
- `--root` beats the pointer file.
- An absolute `plan_root` fails with the message above. A `plan_root` that names
  no directory fails and names the pointer file.
- A subdirectory of the company checkout resolves the same pointer as its root.
- `setup-skills` writes into the company checkout, not the plan repository.
