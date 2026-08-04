---
status: backlog
created: 2026-08-02T13:56:24Z
---
# Multi-repo daemon: one daemon and one UI over N repositories

The daemon is a machine singleton (`~/.plan/daemon.lock`) that serves exactly
one repository: `AppState` (`op-server/src/lib.rs`) holds one `Repo`, one
`Store`, one `Index`, one `IdCounter`, and `serve.rs` builds all of it from the
single `--root`. A second repository on the same machine has nowhere to go —
`ensure_same_repo` (`op-cli/src/writer.rs`) refuses the daemon outright — so
working on two projects means restarting the daemon per project, and the UI
only ever shows one of them.

Make the one daemon serve every repository on the machine: per-project routes,
one SSE stream, a merged all-projects board at `/`, a project switcher in the
UI, and a CLI that registers a repository automatically on its first write.

## Project identity

A registry at `~/.plan/registry.toml`, beside `daemon.json` in the same
`OPLAN_HOME` dir:

```toml
[[project]]
name = "open-plan"
path = "/Users/milansuk/Projects/open-plan"
```

- `path` is the serve root: the main checkout holding a `.plan` store — the
  same anchor `serve_root` picks today (`op-cli/src/writer.rs`), for the same
  reason: it outlives the per-task worktrees this workflow creates and removes.
- `name` is the project's machine-local identity: slugged from the directory
  name at registration, uniquified on collision, renameable. It is deliberately
  **not** the store's abbreviation. The abbreviation is committed in
  `.plan/config.toml`, so two unrelated clones can both claim `APP` and
  nothing local can fix that. Keys stay store-scoped (`OPP-42`); the project is
  an explicit coordinate on routes and URLs; an abbreviation collision is a
  display concern — show the project name beside the key — never an identity
  question.
- The daemon is the registry's only writer. CLI and UI change it through
  `/api/projects`, so registry writes never race across processes.

## What changes, what holds

Per-project failure isolation is the semantic core: a project whose
`config.toml` goes invalid or whose root vanishes is demoted to an error entry
the UI can show — today either one stops the daemon for everything
(`reload_config`, `root_removed` in `op-cli/src/serve.rs`).

What holds, per project, exactly as specified by
[[./00033-daemon-as-single-write-id-author.md]]: keys as the only id spelling,
one id counter per repository, "reads are global, writes are local",
branch-pinned writes, the daemon as the sole in-band writer.

## Sequencing with open work

- [[./00057-route-every-cli-query-through-th.md]] builds a `Reader` mirroring
  `Writer`; it must land after (or be written against) the project model, since
  both resolve "which project is this repo" the same way.
- Presence and rolling updates ([[./00039-continuous-changes-accumulation-v.md]]
  onward) become per-project state when they land; nothing here blocks them.
- The dirty-gated rebuild leans on the watcher, whose known gap is
  [[./00052-watcher-misses-working-tree-edit.md]]; the child task carries the
  mitigation.

The five children are ordered by their dependencies; each leaves the daemon,
CLI, and UI working.
