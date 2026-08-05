---
status: backlog
created: 2026-08-02T13:56:37Z
parent: ./00079-multi-repo-daemon-one-daemon-and.md
dependencies:
- ./00081-serve-n-projects-per-project-wat.md
---
# CLI: oplan project commands and auto-registration on first write

Point the CLI at the multi-project daemon. A write from any repository lands
on the one daemon. The first write registers the repository. A `project`
command group manages the registry.

## `Writer` resolves a project

`Writer::resolve` (`op-cli/src/writer.rs`) keeps its front half: discover the
repo and pin the caller's branch. Replace the back half:

- `ensure_daemon` no longer carries a root that shapes the daemon. The daemon
  starts from the registry. `spawn_detached` (`op-cli/src/daemon.rs`) drops
  `--root <cwd>` from the spawn.
- `--root` stops registering. Delete `name_root` and the registry write from
  `serve.rs`. `oplan project add` and the first write are the only ways to
  register. `--root` keeps one meaning for every command: the directory the
  command works in, as `git -C` does.
- `--root` still names the default project — the project that answers the
  routes with no project segment. The SPA cannot name a project until
  [[./00084-spa-project-routes-switcher-merg.md]], so a definite default must
  hold until then. That child deletes the default with the routes.
- Resolve the project: match the caller's git common directory against
  `GET /api/projects`. On a miss, `POST /api/projects` with the serve root.
  Print one line that names the registered project. `serve_root` stays as the
  anchor computation; the registry records its result.
- Writes go to `/api/projects/{name}/tasks…`. Add the project parameter to
  the task methods in `op-client`. Add methods for the project routes.

Delete `ensure_same_repo`. A daemon that serves other repositories is now the
normal case, not a conflict. Delete `DaemonInfo.repo` with it: `task_updated`
(`op-cli/src/daemon.rs`) resolves the project from `/api/projects` and asks
the prefixed route. It does not compare one recorded repo path.

## `oplan project`

```sh
oplan project list                 # name, root, abbreviation, status
oplan project add [path]           # default: the cwd's serve root
oplan project remove <name>
oplan project rename <old> <new>
```

All four commands route through the daemon. The CLI never touches
`registry.toml`. `remove` says that the files stay on disk. `list` marks a
demoted project with its error reason. The terminal can then answer the
question "why does my UI not show project X".

## Out of scope

The local read paths (`list`, `get`, `show`, `tree` through
`Store::discover`) stay local.
[[./00057-route-every-cli-query-through-th.md]] routes them through the
daemon. Its `Reader` resolves the project exactly as `Writer` does after this
task. That task depends on this one, not the reverse.

## Acceptance

- The first write from an unregistered repository registers it and lands the
  write. The second write prints nothing more.
- Writes from two repositories interleave against one daemon. Each write
  lands in its own store, with its own id sequence.
- Two concurrent first writes from the same repository both succeed and
  register one project. The POST is idempotent.
- `oplan project list` shows both repositories. After a `rename` and a
  refresh, the UI's URLs show the new name.
- A daemon that predates the project routes makes the CLI fail with the
  existing restart-the-daemon message, not a panic.
- `cargo build && cargo test && cargo fmt --check && cargo clippy -- -D warnings` pass.
