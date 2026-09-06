---
status: backlog
created: 2026-07-26T15:40:55Z
tags:
- feature
- git
---
# Continuous changes accumulation via the rolling-updates branch

Ambient edits are the task edits that belong to no feature branch. This is the
umbrella for the lane that holds them.

Ambient edits accumulate on the branch `openplan/updates`. The daemon keeps a
standing worktree for it, rebases it onto the default branch as that branch
moves, and fast-forwards the default branch when a person publishes.

## Why the storage changed

[[./00023-design-a-continuous-changes-accu.md]] designed the first version. It
kept the lane in the custom ref `refs/open-plan/rolling-updates` and wrote git
objects with no worktree. Three findings stopped it:

- gix hosts a merge driver only as an external command, and it turns a non-zero
  exit into an error rather than a conflict. The worktree-less reconcile could
  not name the conflicted tasks.
- `Index` builds its lanes from `repo.local_branches()`, so a custom ref never
  reached the read path at all.
- A conflict landed in a ref with no files on the disk, so nobody could resolve
  it.

A real branch with a worktree answers all three. Reads, cross-process write
locking, and conflict resolution all come from code that already exists. The
spike in [[./00023-design-a-continuous-changes-accu.md]] proved that
`git rebase` calls the merge driver, and this design uses that mechanism.

## Children

- Backend and CLI: [[./00109-rolling-updates-an-ambient-edit-b.md]]
- UI: [[./00110-ui-rolling-updates-sync-control-a.md]]
