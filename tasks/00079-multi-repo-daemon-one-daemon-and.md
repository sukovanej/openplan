---
status: done
created: 2026-08-02T13:56:24Z
---
# Multi-repo daemon: one daemon and one UI over N repositories

The daemon is a machine singleton (`~/.plan/daemon.lock`). It serves one
repository only. `AppState` (`op-server/src/lib.rs`) holds one `Repo`, one
`Store`, one `Index`, and one `IdCounter`. `serve.rs` builds all of them from
the one `--root`. A second repository on the same machine has no daemon that
accepts it: `ensure_same_repo` (`op-cli/src/writer.rs`) refuses a daemon that
serves a different repository. You must restart the daemon for each project,
and the UI shows one project only.

Make the one daemon serve all repositories on the machine. Give it
per-project routes, one SSE stream, and a merged all-projects board at `/`.
Give the UI a project switcher. Make the CLI register a repository on its
first write.

## Project identity

Add a registry at `~/.plan/registry.toml`. Put it adjacent to `daemon.json`,
in the same `OPLAN_HOME` directory:

```toml
[[project]]
name = "open-plan"
path = "/Users/milansuk/Projects/open-plan"
```

- `path` is the serve root: the main checkout that holds a `.plan` store.
  This is the same anchor that `serve_root` (`op-cli/src/writer.rs`) selects
  today, for the same reason: it stays when this workflow creates and removes
  a worktree for each task.
- `name` is the machine-local identity of the project. The daemon makes it
  from the directory name at registration. The daemon makes the name unique
  on a collision. The user can rename it. The name is **not** the store's
  abbreviation, and this is intentional: the abbreviation is committed in
  `.plan/config.toml`, so two unrelated clones can both claim `APP`, and no
  local change can correct that. Keys stay store-scoped (`OPP-42`). The
  project is an explicit coordinate on routes and URLs. An abbreviation
  collision is a display concern: show the project name adjacent to the key.
  It is never an identity question.
- The daemon is the only writer of the registry. The CLI and the UI change it
  through `/api/projects`. Registry writes then never race across processes.

## What changes, and what holds

Per-project failure isolation is the semantic core. When a project's
`config.toml` becomes invalid, the daemon demotes that project to an error
entry. When a project's root is deleted, the daemon does the same. The UI can
show the error entry. Today, each of these two conditions stops the daemon
for everything (`reload_config` and `root_removed` in `op-cli/src/serve.rs`).

These rules hold for each project, as
[[./00033-daemon-as-single-write-id-author.md]] specifies: keys are the only
id spelling; each repository has one id counter; reads are global and writes
are local; a write is pinned to a branch; the daemon is the only in-band
writer.

## Sequence with open work

- [[./00057-route-every-cli-query-through-th.md]] adds a `Reader` that
  mirrors `Writer`. It must come after the project model, or be written
  against it. Both answer the same question: which project is this
  repository?
- Presence and rolling updates
  ([[./00039-continuous-changes-accumulation-v.md]] onward) become
  per-project state when they land. This task does not block them.
- The dirty-gated rebuild uses the watcher. The watcher has a known gap:
  [[./00052-watcher-misses-working-tree-edit.md]]. The child task contains
  the mitigation.

The five children are in dependency order. Each child keeps the daemon, the
CLI, and the UI in a working state.
