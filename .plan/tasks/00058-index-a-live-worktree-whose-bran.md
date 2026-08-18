---
status: backlog
created: 2026-07-30T12:18:53Z
---
# Index a live worktree whose branch has no ref yet

The index is built by walking `refs/heads/*`, so a repository with no commits
yet contributes no matrix cells at all — and every read the daemon answers
starts from a cell. A freshly `git init`ed project is invisible to the daemon
even though its task files are sitting in the worktree the daemon can see.

## Today

`Index::rebuild` (`op-index/src/lib.rs`) takes `repo.local_branches()` and builds
one matrix cell per (task, branch) from each branch's tip. An unborn HEAD — the
state right after `git init`, where HEAD names a branch that has no ref — yields
an empty branch list, so the matrix is empty. `self.live` does record the
worktree, and `max_id` counts its files, but nothing else consults them:

- `branch_summaries(branch)` filters `matrix.cells`, so it returns nothing.
- `effective_raw` and `effective_view` return `None` the moment `self.cell(id,
  branch)` misses, without ever asking `self.live` — the dirty-overlay read only
  happens *after* a cell is found.

Writes work in this state, because `Writer` resolves the store, not the index.
Reads do not.

## Why it matters now

[[./00057-route-every-cli-query-through-th.md]] makes the daemon the only resolver
for reads, which turns this from a latent index gap into user-visible behaviour:
`openplan list` and `openplan get` in a just-initialized project report no tasks
instead of reading the files. That task deliberately accepted the regression
rather than widen into `op-index`, and gave its CLI test harness
(`Project::new`, `op-cli/tests/cli.rs`) a commit so the suite runs against a
born branch — the same workaround `op-server`'s harness already uses with its
birthing commit.

Local-first means the tool works before you have decided to commit anything.

## Change

Emit matrix cells for a live worktree whose branch has no ref, sourced from the
worktree's files rather than a blob. The pieces that need to agree:

- The cell needs a `ChangeKind` and a `blob_oid` it cannot have. Decide whether
  an unborn branch's cell is `Added` with a synthetic/absent oid, or whether
  `MatrixCell` grows a variant for "lives only in the working tree".
- Such a cell is always `dirty`, so `effective_raw`'s existing dirty path reads
  it from `self.live` and needs no change once the cell exists.
- `base_blobs_by_branch` and `compute_headlines` must tolerate a branch with no
  commit and no merge-base.
- `default_branch` is `None` here, which the base-blob path already handles.

## Verify

- In a repository with `git init` and no commits, `openplan list` and `openplan get
  <key>` return the worktree's tasks through the daemon.
- `openplan list --all-branches` shows the unborn branch's tasks as living only in
  the working tree, not as committed on a branch.
- After the first commit, the same task reports one branch and is no longer
  dirty — no duplicate row across the unborn and born states.
- Reverting the harness commit added by
  [[./00057-route-every-cli-query-through-th.md]] leaves `op-cli`'s tests
  passing.
- `cargo build && cargo test && cargo fmt --check && cargo clippy -- -D warnings` pass.
