---
status: backlog
created: 2026-08-02T13:56:37Z
parent: ./00079-multi-repo-daemon-one-daemon-and.md
dependencies:
- ./00081-serve-n-projects-per-project-wat.md
---
# CLI: oplan project commands and auto-registration on first write

Point the CLI at the multi-project daemon: a write from any repository lands on
the one daemon, registering the repository on its first write, and a `project`
command group manages the registry.

## `Writer` resolves a project, not a daemon-wide repo

`Writer::resolve` (`op-cli/src/writer.rs`) keeps its front half — discover the
repo, pin the caller's branch — and replaces the back half:

- `ensure_daemon` no longer carries a root that shapes the daemon; the daemon
  starts from the registry. `spawn_detached` (`op-cli/src/daemon.rs`) drops
  `--root <cwd>` from the spawn; `server start --root` survives as the explicit
  "and register this" form.
- Resolve the project by matching the caller's git common dir against
  `GET /api/projects`. On a miss, `POST /api/projects` with the serve root and
  print one line naming the registered project. `serve_root` stays as the
  anchor computation; the registry records its result.
- Writes go to `/api/projects/{name}/tasks…`; `op-client` grows the project
  parameter on the task methods and methods for the project routes.

`ensure_same_repo` is deleted — a daemon serving other repositories is now the
normal case, not a conflict. `DaemonInfo.repo` goes with it: `task_updated`
(`op-cli/src/daemon.rs`) resolves the project from `/api/projects` and asks the
prefixed route instead of comparing one recorded repo path.

## `oplan project`

```sh
oplan project list                 # name, root, abbreviation, status
oplan project add [path]           # default: the cwd's serve root
oplan project remove <name>
oplan project rename <old> <new>
```

All four route through the daemon — the CLI never touches `registry.toml`.
`remove` states that files are untouched. `list` marks a demoted project with
its error reason, so "why is my UI missing project X" is answerable from the
terminal.

## What this child does not touch

The local read paths (`list`, `get`, `show`, `tree` via `Store::discover`) stay
local; routing them through the daemon is
[[./00057-route-every-cli-query-through-th.md]], whose `Reader` resolves the
project exactly as `Writer` now does. That task depends on this one, not the
reverse.

## Acceptance

- First write from an unregistered repository registers it and lands the write;
  the second write says nothing extra.
- Writes from two repositories interleave against one daemon; each lands in its
  own store with its own id sequence.
- Two concurrent first writes from the same repository both succeed and
  register one project (the POST is idempotent).
- `oplan project list` shows both repositories; `rename` is reflected in
  the UI's URLs after a refresh.
- A daemon predating the project routes fails the CLI with the existing
  restart-the-daemon wording, not a panic.
- `cargo build && cargo test && cargo fmt --check && cargo clippy -- -D warnings` pass.
