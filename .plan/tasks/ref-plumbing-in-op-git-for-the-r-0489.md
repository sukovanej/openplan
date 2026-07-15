---
status: todo
deps:
- implement-the-section-aware-merg-381e
- design-a-continuous-changes-accu-2380
---
# Ref plumbing in op-git for the rolling-updates ref

**Phase 1** of the rolling-updates plan
([[design-a-continuous-changes-accu-2380]] §7.11). Gives `op-git` its first
**write** path (today it is read-only: "Raw git reads"), scoped to the single
`refs/open-plan/rolling-updates` ref. Depends on the section-merge **library**
from [[implement-the-section-aware-merg-381e]].

## Precondition spike (do first)

Prove `gix::merge::tree` (gix 0.85, `merge` feature) can perform a path-level
3-way tree merge **hosting our own blob-merge resolver** (the extracted section
merge). If it cannot, that is a design blocker to resolve here (bump gix /
contribute the hook) — **not** a runtime fallback. There is no `git merge-tree`
shell-out anywhere in this crate.

## API (new `rolling.rs` module on `Repo`)

```rust
pub const ROLLING_REF: &str = "refs/open-plan/rolling-updates";

pub fn read_ref(&self, name: &str) -> Result<Option<String>, GitError>;
pub fn update_ref(&self, name: &str, new: &str, expected: Option<&str>)  // CAS
    -> Result<(), GitError>;                                             // Err(RefRace) on mismatch

// worktree-less accumulation: overlay task-file changes onto parent's tree, commit.
// changes: path -> Some(bytes) to write/replace, None to delete.
pub fn commit_overlay(
    &self, parent: &str, changes: &[(String, Option<Vec<u8>>)], message: &str,
) -> Result<String, GitError>;

// reconcile primitive — in-process only, via gix::merge::tree + the section-merge lib.
pub enum Reconciled { Clean { tree: String }, Conflicts { tasks: Vec<String> } }
pub fn merge_trees(&self, base: &str, ours: &str, theirs: &str)
    -> Result<Reconciled, GitError>;

// replay pending ambient commits onto `onto`, one merge_trees step each, linear stack.
pub enum Replayed { Done { tip: String }, Blocked { last_good: String, tasks: Vec<String> } }
pub fn replay_onto(&self, onto: &str, commits: &[String]) -> Result<Replayed, GitError>;

// publish
pub fn is_fast_forward(&self, from: &str, to: &str) -> Result<bool, GitError>;
pub fn fast_forward(&self, name: &str, to: &str) -> Result<(), GitError>; // Err(NotFastForward)
```

- `merge_trees` outcomes: clean auto-merge → `Clean`; genuine same-section
  overlap → `Conflicts` (expected, not an error); merge machinery unable to run
  (resolver can't be hosted, unreadable object) → **`Err`**, propagated. Never a
  shell-out.
- New `GitError` variants: `RefRace`, `NotFastForward`, `Conflict`.
- Reuse existing `merge_bases_against` to pick the base for `merge_trees` /
  `replay_onto`.
- Update the crate `description` in `Cargo.toml` — it no longer reads-only.

No daemon wiring in this phase (that is Phase 2).

## Verify

Unit tests (`crates/op-git/tests/`) drive a temp repo through:
- accumulate: `commit_overlay` advances the ref via `update_ref` CAS; a stale
  `expected` yields `RefRace`;
- reconcile-clean: non-overlapping section/field edits → `Clean` merged tree;
- reconcile-conflict: same-section divergence → `Conflicts { tasks }` naming the
  logical ids;
- replay preserves a linear stack and stops at `Blocked` on the first conflict;
- publish: `fast_forward` advances a ref; a non-FF target yields `NotFastForward`
  (never a merge).
