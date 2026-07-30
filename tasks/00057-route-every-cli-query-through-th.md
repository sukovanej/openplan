---
status: backlog
created: 2026-07-30T11:05:18Z
---
# Route every CLI query through the daemon; drop local read fallbacks

Make the daemon the single resolver for **reads** as well as writes, so a query
answers identically whether it came from the CLI or the web UI. There is no local
fallback: if the daemon is not running the CLI starts it, and if it cannot be
started the command fails. Reads stop degrading — they behave exactly like writes
already do after [[./00033-daemon-as-single-write-id-author.md]].

## Today — the write half shipped, the read half never did

[[./00033-daemon-as-single-write-id-author.md]] routed `create`/`set`/`delete`
through the HTTP API via `Writer::resolve` (`op-cli/src/writer.rs`), which
discovers the repo, pins the caller's branch, auto-starts the daemon, and refuses
a daemon serving a different repository. Queries never got the same treatment, so
the CLI runs two unrelated resolvers side by side:

| Path | Resolver |
|---|---|
| `list` / `get` / `show` / `tree` (no branch flag) | `Store::discover` — raw files in the cwd's worktree |
| `get --branch`, `list --all-branches`, `show --branches` | `op_index::Index` built one-shot in-process |
| Web UI (`/api/tasks`, `/api/board`, `/api/tasks/{id}`) | `op_index::Index` held warm by the daemon |

Both CLI resolvers are local, so this is not a transport gap — the default query
paths simply skip the branch-aware index the daemon and the UI share. The visible
symptom: a task committed on no branch but live in another worktree is found by
`oplan list --all-branches` and by the UI, while `oplan get <key>` reports `no such
task` and says nothing about where the task actually lives.

`op-cli/src/main.rs:777` already asserts the intended rule — "reads are global,
writes go through the daemon" — in a comment directly above a `Store::discover`
call that is worktree-local.

## The rule

The daemon is the only process that reads or writes the store in band. A CLI
command resolves the daemon the way `Writer` does, and on failure it exits with
that failure — never a local read.

Two exemptions, both because no daemon can be assumed to exist at all:

- `oplan merge-driver`, which git invokes mid-merge. It is a pure text merge over
  the three paths git hands it and touches neither store nor index today
  (`op-cli/src/mergedriver.rs`); keep it that way.
- `oplan server start|stop|restart|status`, which manage the daemon's lifetime and
  cannot presuppose it.

`op-lint` ([[./00055-lint-task-files-op-lint-crate-an.md]]) is a filesystem check
over the worktree in front of the caller, not a task query; settle whether it
reads the store directly when that command lands.

## Every read carries the caller's branch

A branchless read on the daemon resolves against the **serve root's** branch
(`write_branch`, `op-server/src/lib.rs`), and the serve root is anchored at the
main checkout so it survives worktree removal (`serve_root`, `op-cli/src/writer.rs`).
A CLI that omits `?branch=` would therefore start answering `oplan get <key>` inside
a worktree with *main's* version of the task — a silent wrong-branch read, the
mirror of the wrong-branch write that pinning the branch prevents. Every read sends
`repo.current_branch()`, exactly as writes do.

This deliberately does not make a branch-scoped query answer across branches:
`oplan get <key>` from a branch that lacks the task still fails. Consistency here
means one resolver, not one answer — `get` asks about a branch, the board
aggregates. What changes is that the daemon knows every branch a task lives on
(`Index::task_branch_states`), so the failure can name them instead of claiming the
task does not exist.

## Change

**`Reader`, mirroring `Writer`** (`op-cli/src/writer.rs` alongside it): discover the
repo, take `current_branch()`, `ensure_daemon` at the main-checkout serve root,
`ensure_same_repo`. Unlike `Writer` it has no branchless mode; unlike today's read
paths it has no local mode.

**Four missing routes.** The response types already exist in `op-api` and the index
methods behind them are already `pub`; only the HTTP surface is absent:

| Route | Index method | CLI command |
|---|---|---|
| `GET /api/tasks?branch=` | `branch_summaries` | `list`, `list --branch` |
| `GET /api/matrix` | `matrix` | `list --all-branches` |
| `GET /api/tasks/{id}/tree` | `hierarchy_context` | `tree` |
| `GET /api/tasks/{id}/branches` | `task_branches` | `show --branches` |

Regenerate the web client (`mise run generate-web-client`) since the OpenAPI spec
grows.

**`op-client`**: read methods for the four routes above plus the existing
`GET /api/tasks/{id}`.

**Delete the local read paths** in `op-cli/src/main.rs`: `Store::discover` in
`list`, `get`, `show`, `tree`, `delete`'s confirmation read, and `move`'s sibling
read; `build_index` and its callers `list_all_branches`, `list_branch`,
`get_branch`, `show_branches`; and `local_summaries`. `build_index` itself should be
gone, not merely unused. `branches` reads `repo.local_branches()` rather than the
store — either route it through the daemon for consistency or state why the git ref
listing is exempt.

**`local_updated`** already prefers the daemon's control channel and falls back to a
one-shot cross-branch index. Drop the fallback; the field comes from the daemon or
it is `Missing`.

**`move`** reads its sibling group locally to compute ranks, then writes through the
daemon — a read-modify-write split across two resolvers. Take the sibling group from
the daemon so the ranks are computed from the same state the write lands on.

**SPEC.md §7.3** documents the opposite of this task: "if the daemon is down, the CLI
reads files / the object DB directly … **Reads** degrade; **writes** do not."
Rewrite that paragraph — neither degrades, and the auto-start/fail-loudly rule now
covers both. Keep the existing note that auto-start roots the daemon at the main
checkout. Check §6 and the §7.1 "reads are global, writes are local" invariant for
wording that now reads as a fallback licence; the invariant itself still holds (a
read may span branches, a write may not) and should survive.

## Failure modes to get right

The point of removing the fallback is that a broken daemon is *reported*, so each
of these needs a message that names the cause and the fix:

- daemon down and auto-start fails — surface the log path, as `ensure_daemon` does
- daemon serving another repository — `ensure_same_repo`'s existing wording
- detached HEAD, so no branch to read — writes already fail here; reads now do too
- not a git repository, or no `.plan/` store found
- task absent on the caller's branch but present elsewhere — name those branches

## Acceptance

- No `Store::discover`, `Index::rebuild`, or `Repo`-based task read remains in
  `op-cli` outside `merge-driver` and the `server` lifecycle commands.
- `list`, `get`, `show`, `tree`, `move`, and `delete` produce byte-identical output
  to today for a task on the caller's branch, with the daemon running.
- With the daemon stopped and auto-start sabotaged, every one of those commands
  exits non-zero with a message naming the daemon as the cause — none prints task
  data.
- `oplan get <key>` for a task that exists only on another branch names that branch
  and its dirty state; `--branch <other>` prints it.
- A read from a linked worktree returns that worktree's branch's version, not the
  serve root's.
- SPEC.md carries no remaining claim that reads degrade to local access.
- `cargo build && cargo test && cargo fmt --check && cargo clippy -- -D warnings` pass.
