---
status: backlog
created: 2026-07-29T10:05:59Z
tags:
- bug
- daemon
---
# Restore the last-change walk's early exit and stop re-reading trees

## Goal

Dating every task cost the last-change walk its early exit. It now descends
essentially the whole history on every rebuild, reading each commit's task tree
twice, under the index mutex. Give it back a reason to stop, and stop paying for
the same tree more than once.

## The regression

`Index::compute_change_times` used to ask `op_git::task_change_times` about the
*contested* ids only — a task living on several branches with differing blobs,
usually a handful. The walk's `needed` set emptied within a few commits and it
quit there.

Every non-deleted, non-dirty cell is now requested, because every cell reports an
`updated`. On the default branch that is every task, so `needed` can only empty
at the commit that introduced the *oldest* task: the walk descends the whole
history. Per commit it calls `commit_task_map` for the commit and again for each
parent, with no memoisation, so every tree is read twice — once as itself, once
as its child's parent — and each read lists the whole `.plan/tasks/` directory.

Measured on this repo (46 tasks, 138 commits, debug build): `openplan show` 0.007s,
`openplan get --json` 0.087s — roughly 276 tree reads to fill one field. It is
O(commits x tasks), so it grows with the history, and the daemon pays it again
on the first request after any branch tip moves, holding the index mutex while
every other read waits.

## Scope

- Memoise `commit_task_map` by commit id for the length of a walk. A commit
  visited as itself and then as its child's parent is two reads of one tree.
- Restore an early exit. A task's date is settled the moment its last change is
  seen, which `needed` already tracks — the problem is only that `needed` now
  starts out holding everything. Options worth weighing: ask per branch only for
  the ids that branch can answer for (a non-baseline branch's cells are few),
  date the default branch's tasks from a walk that is incremental against the
  previous tip rather than restarted, or keep a persistent per-commit map so a
  rebuild after one new commit reads one tree.
- Whatever the shape, the daemon must not re-walk the whole history on the first
  request after every commit.

## `HISTORY_WALK_BUDGET`

The 4096-commit cap is not new — it arrived with the headline logic in #17 — but
what it costs did change, and its comment is now the only thing that says so.

It was written for a walk that fed ranking alone, where an undated id merely
sorts as oldest and "only ever lets a more recent branch outrank it". That id is
now also a task whose `updated` is absent, with nothing said about why. Since
the walk no longer exits early, a repo past 4096 commits will reach it routinely
rather than never.

Decide whether the cap should stay once the walk is bounded properly, and if it
stays, whether a task it could not reach should report that as a reason rather
than as silence — `updated` already carries a `FieldError::Invalid` for a commit
git could not date, and this is the same kind of "we could not find out".

## Done when

- A rebuild after one new commit does not re-read history the previous rebuild
  already read.
- No tree is read twice within one walk.
- `openplan get --json` on a repo with thousands of commits is not dominated by the
  walk.
- The budget is either gone, or reachable only in cases the field explains.
