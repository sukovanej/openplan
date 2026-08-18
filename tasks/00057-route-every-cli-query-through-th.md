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
`openplan list --all-branches` and by the UI, while `openplan get <key>` reports `no such
task` and says nothing about where the task actually lives.

`op-cli/src/main.rs:777` already asserts the intended rule — "reads are global,
writes go through the daemon" — in a comment directly above a `Store::discover`
call that is worktree-local.

## The rule

The daemon is the only process that reads or writes the store in band. A CLI
command resolves the daemon the way `Writer` does, and on failure it exits with
that failure — never a local read.

Two exemptions, both because no daemon can be assumed to exist at all:

- `openplan merge-driver`, which git invokes mid-merge. It is a pure text merge over
  the three paths git hands it and touches neither store nor index today
  (`op-cli/src/mergedriver.rs`); keep it that way.
- `openplan server start|stop|restart|status`, which manage the daemon's lifetime and
  cannot presuppose it.

`op-lint` ([[./00055-lint-task-files-op-lint-crate-an.md]]) is a filesystem check
over the worktree in front of the caller, not a task query; settle whether it
reads the store directly when that command lands.

## Every read carries the caller's branch

A branchless read on the daemon resolves against the **serve root's** branch
(`write_branch`, `op-server/src/lib.rs`), and the serve root is anchored at the
main checkout so it survives worktree removal (`serve_root`, `op-cli/src/writer.rs`).
A CLI that omits `?branch=` would therefore start answering `openplan get <key>` inside
a worktree with *main's* version of the task — a silent wrong-branch read, the
mirror of the wrong-branch write that pinning the branch prevents. Every read sends
`repo.current_branch()`, exactly as writes do.

This deliberately does not make a branch-scoped query answer across branches:
`openplan get <key>` from a branch that lacks the task still fails. Consistency here
means one resolver, not one answer — `get` asks about a branch, the board
aggregates. What changes is that the daemon knows every branch a task lives on
(`Index::task_branch_states`), so the failure can name them instead of claiming the
task does not exist.

## Change

**`Reader`, mirroring `Writer`** (`op-cli/src/writer.rs` alongside it): discover the
repo, take `current_branch()`, `ensure_daemon` at the main-checkout serve root,
`ensure_same_repo`. Unlike `Writer` it has no branchless mode; unlike today's read
paths it has no local mode.

**Four missing routes**, plus a branch parameter on the two that exist:

| Route | Index method | CLI command |
|---|---|---|
| `GET /api/tasks?branch=` | `branch_tasks` (new) | `list`, `list --branch` |
| `GET /api/matrix` | `matrix` | `list --all-branches` |
| `GET /api/tasks/{id}/tree?branch=` | `branch_summaries` + `TaskTree::build` | `tree` |
| `GET /api/tasks/{id}/branches` | `task_branches` | `show --branches` |
| `GET /api/tasks/{id}?branch=` | `effective_view` | `get`, `show` |

Two of these need more than an HTTP handler, and the shapes below are the design —
the CLI bends to the daemon's answer, not the reverse:

- **`GET /api/tasks?branch=` keeps one response schema in both forms**:
  `Vec<TaskListItem>`, exactly what the SPA's task list already decodes. Branchless
  stays `aggregated_tasks` — one row per logical task across every branch. With
  `?branch=` a new `Index::branch_tasks` returns that branch's cells as
  `TaskListItem`, `headline` being the branch asked for and `branches` holding only
  its own state. `list --json` therefore prints `TaskListItem`, not `TaskSummary`:
  its JSON grows `updated`, `headline`, and `branches`. That output change is the
  point — CLI and UI read one route with one shape. The human-readable table keeps
  its current columns, and `TaskSummary` survives only where the matrix already
  uses it.
- **`GET /api/tasks/{id}/tree` needs a response type that does not exist yet.**
  `hierarchy_context` answers a different question (parent title, *direct*
  children, body refs — what `TaskDetail` embeds for the UI). `openplan tree` prints a
  recursive, `--depth`-bounded `TaskTree` whose nodes carry full `Metadata`, plus
  one stderr warning per parent cycle it truncated. Add the response type to
  `op-api` carrying the `TaskTree` and the truncated ids, and build it from
  `branch_summaries` the way `tree` builds it locally today.

Regenerate the web client (`mise run generate-web-client`) since the OpenAPI spec
grows.

**`op-client`**: read methods for every route in the table above.

**`get` prints the daemon's `TaskDetail`, re-serialized — not the file's bytes.**
Today it prints the file verbatim, because the local path has the bytes in hand and
re-serializing would normalize formatting (`op-cli/src/main.rs`, pinned by
`get_prints_the_file_verbatim`). No route reconstructs those bytes, and none should:
the daemon answers with parsed state, so `get` renders that state back to markdown —
canonical frontmatter from `Metadata`, `# title`, then `body`. Frontmatter key order,
spacing, and unknown keys normalize. Retire the verbatim test in favour of one that
asserts the normalized rendering, and drop `get --branch`'s separate raw path with
it. `get --json` prints `TaskDetail` for the same reason `list --json` prints
`TaskListItem`.

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

Neither reads nor writes degrade any more — the auto-start/fail-loudly rule covers
both. Auto-start still roots the daemon at the main checkout. The "reads are global,
writes are local" invariant survives unchanged: a read may span branches, a write may
not.

## Out of scope: a repository with no commits

`Index::rebuild` walks `refs/heads/*`, so a repository whose HEAD is unborn has no
branches, no matrix cells, and therefore no daemon-answerable reads — every read
starts from a cell, and `effective_raw` returns `None` before it ever consults
`self.live`. Local reads work there today; routed through the daemon they stop.

Closing that gap belongs to [[./00058-index-a-live-worktree-whose-bran.md]], not
here. This task accepts the regression and gives `op-cli/tests/cli.rs` its own
birthing commit — `Project::new` never commits, so nearly every CLI test runs on an
unborn branch — matching what `op-server`'s harness already does. Do not work around
it per-test, and do not smuggle worktree indexing into this change.

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
  `op-cli` outside `merge-driver` and the `server` lifecycle commands. `serve_root`
  in `writer.rs` keeps its `Store::discover` — it anchors the daemon, it is not a
  task read — and `branches` keeps `Repo::local_branches()` with a stated why.
- `show`, `tree`, `move`, and `delete` produce byte-identical output to today for a
  task on the caller's branch, with the daemon running.
- `get` and `list` produce today's output up to the two deliberate changes above:
  `get`'s markdown is normalized rather than verbatim, and `list --json` prints
  `TaskListItem`. Nothing else about either shifts — the human-readable table's
  columns, ordering, `--status`/`--parent` filtering, and empty-state lines hold.
- With the daemon stopped and auto-start sabotaged, every one of those commands
  exits non-zero with a message naming the daemon as the cause — none prints task
  data.
- `openplan get <key>` for a task that exists only on another branch names that branch
  and its dirty state; `--branch <other>` prints it.
- A read from a linked worktree returns that worktree's branch's version, not the
  serve root's.
- `list --branch <unknown>` exits non-zero naming the branch — it does not degrade
  into a successful empty list now that `ensure_branch` is gone, which means the
  daemon rejects an unknown `?branch=` rather than answering with no cells.
- The SPA is untouched by the branchless `/api/tasks` response, and the regenerated
  client's existing decode tests pass unchanged.
- `cargo build && cargo test && cargo fmt --check && cargo clippy -- -D warnings` pass.
