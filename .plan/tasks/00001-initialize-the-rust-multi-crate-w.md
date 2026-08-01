---
status: done
created: 2026-07-13T21:19:38Z
---

# Initialize the Rust multi-crate workspace

## Goal
Stand up the open-planner Cargo workspace: a compiling skeleton of every crate with its
responsibilities, dependency edges, and binaries wired, so feature work can start against a
stable structure. Skeleton only — no domain logic yet.

## Workspace layout
```
open-planner/
├── Cargo.toml                 # [workspace] + [workspace.dependencies] + shared lints
├── rust-toolchain.toml        # pinned stable toolchain
├── .plan/                     # this store (dogfooding)
├── crates/
│   ├── op-md/                 # lib — markdown section/block addressing + splice edits
│   ├── op-task/               # lib — task file model: frontmatter + body, id/title conventions
│   ├── op-store/              # lib — .plan/ filesystem store: CRUD, atomic writes, locking
│   ├── op-git/                # lib — raw git reads: worktrees, refs, object-db, diff-tree
│   ├── op-api/                # lib — shared wire types (matrix, presence, change events)
│   ├── op-watch/              # lib — fs/git watchers → debounced change events
│   ├── op-index/              # lib — aggregation engine: maintains the task×branch matrix
│   ├── op-presence/           # lib — ephemeral claim/coordination registry
│   ├── op-server/             # lib — axum HTTP + WebSocket API + embedded SPA
│   └── op-cli/                # bin `oplan` — single binary: CLI + `serve` daemon + `merge-driver`
└── web/                       # frontend SPA (non-cargo), embedded into op-server
```

## Crates
Each crate has one job — no catch-all core, no catch-all daemon.

**Model & storage**
- **op-md** (lib): the addressing primitive — parse a markdown body to a tree, resolve `-t`
  target paths (section / nested / positional block) to byte ranges, and splice-edit a target
  without reflowing the rest. Pure; no task or filesystem knowledge.
  Deps: `comrak` (sourcepos) or `pulldown-cmark` (OffsetIter).
- **op-task** (lib): the task file model — frontmatter (`status` / `parent` / `deps`) + body,
  the conventions (id = filename, title = single `# H1`), parse ↔ serialize. Deps: `op-md`, `serde`, `serde_yaml`.
- **op-store** (lib): the on-disk store — locate `.plan/`, list/read/write task files, atomic
  temp+rename writes, advisory per-file locking. Deps: `op-task`, `fs2`/`fd-lock`.
- **op-git** (lib): raw git reads — worktree enumeration, ref + object-DB reads (trees/blobs by
  OID), `diff-tree`, git-op-in-progress detection. No task parsing. Deps: `gix` (gitoxide).
- **op-api** (lib): the shared wire contract — serde types for tasks, the matrix, presence, and
  change/live events, consumed by every daemon crate + cli + web. Deps: `op-task`, `serde`.

**Daemon (split into targeted crates)**
- **op-watch** (lib): filesystem + git watchers — working trees, refs, `.git/worktrees/`;
  debounce; settle on in-progress git ops; emit `op-api` change events. Deps: `op-git`, `op-api`, `notify`.
- **op-index** (lib): the aggregation engine — consume op-store (working tree) + op-git
  (committed blobs) driven by op-watch events; parse blobs into tasks, cache by blob OID,
  maintain the live task×branch matrix. Deps: `op-store`, `op-git`, `op-task`, `op-api`.
- **op-presence** (lib): the ephemeral claim/coordination registry — claim / release /
  heartbeat / expiry, machine-local `claims.json` (file-locked). Deps: `op-api`, `fs2`/`fd-lock`.
- **op-server** (lib): the `axum` HTTP + WebSocket API over op-index + op-presence; pushes live
  events; serves the embedded SPA (`rust-embed`). Deps: `op-index`, `op-presence`, `op-api`, `axum`, `tokio`, `rust-embed`.

**Binary — one `oplan`**
- **op-cli** (bin `oplan`): the single binary; each face is a subcommand rather than a separate bin:
  - the **CLI surface** — talks to the daemon over HTTP, falls back to op-store/op-git
    directly when it's down;
  - **`serve`** — the singleton daemon (process lifecycle, project registry; wires watch + index +
    presence + server);
  - **`merge-driver`** — the `.gitattributes` driver for `.plan/**.md`; 3-way merge at
    frontmatter-field + section granularity.
  Deps: `op-store`, `op-git`, `op-api`, `op-watch`, `op-index`, `op-presence`, `op-server`,
  `op-md`, `op-task`, `clap`, `reqwest`, `tokio`.

## Shared workspace config
- Edition **2024**; toolchain pinned in `rust-toolchain.toml` (recent stable).
- Common deps hoisted to `[workspace.dependencies]`; each crate uses `{ workspace = true }`.
- Lints: `cargo fmt --check` and `cargo clippy -- -D warnings` clean.

## Acceptance criteria
- [x] `cargo build` succeeds for the whole workspace (all 10 crates, stub content).
- [x] `cargo run -p op-cli -- --version` and `--help` work; the binary is named `oplan`.
- [x] `cargo run -p op-cli -- serve` starts an axum server exposing `GET /health` → `200`.
- [x] `cargo run -p op-cli -- merge-driver <O> <A> <B>` runs (exit `0` identical, non-zero divergent).
- [x] `op-task` exposes stub `Task` + `Status` and passes a frontmatter parse↔serialize roundtrip test.
- [x] `cargo fmt --check` and `cargo clippy -- -D warnings` pass.
- [x] `web/` has a placeholder the `oplan` binary embeds (an empty `index.html` is fine for now).
- [x] `README.md` documents build/run.

## Out of scope (this task)
Real addressing/store/git/daemon logic, the merge algorithm, and the actual UI all follow as
child tasks. This task delivers the compiling skeleton and dependency wiring only.

## Notes
- `oplan` chosen over `op` to avoid the 1Password CLI clash.
- **One binary, not three:** `serve` (daemon) and `merge-driver` are subcommands of `oplan`, not
  separate `op-daemon`/`op-mergedriver` binaries. Daemon/merge glue lives in `op-cli`; the reusable
  logic stays in the library crates (`op-server`, `op-md`, `op-task`, …).
- The `web/` frontend framework is TBD (deferred); a static placeholder unblocks embedding.
- Wire types live in `op-api`, shared by daemon, cli, and web.
