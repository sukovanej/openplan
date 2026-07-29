---
status: backlog
created: 2026-07-29T10:33:40Z
---
# Date a local read by the branch whose bytes it printed

## Problem

`oplan get <id> --json` prints the file in the caller's worktree and reports an
`updated` alongside it. When the checked-out branch has no cell for that task —
its copy is unchanged from the merge-base, so the index records no divergence —
there is no date to read there, and `Index::task_updated_or_headline` falls back
to `task_updated(id, None)`: the newest cell on *any* branch. That dates the read
by an edit on a branch whose content it never showed.

## Reproduction

Three branches. `main` holds the task; `feat-x` edits it later; `mine` forks from
`main` and never touches it.

```
2026-05-28  feat-x   status: in_progress
2025-02-19  main     status: todo          <- `mine` points here too
```

From the `mine` worktree:

```
$ cat .plan/tasks/shared-0001.md
status: todo

$ oplan get shared-0001 --json
status:  todo                     # main's content, from Feb 2025
updated: 2026-05-28T20:26:40Z     # feat-x's commit
```

Nothing in what was printed changed on that date, and the version that did change
is not in the output.

The aggregated views are not affected: the board headlines `feat-x` and shows
*its* content with *its* date, which agree. Only a branch-scoped read mismatches.

## Why the daemon does not solve it

`GET /api/tasks/{id}?branch=mine` is a 404 — `effective_raw` returns `None` when
the branch has no cell, which predates this and is unrelated to timestamps. So
the CLI's daemon path bails and lands on the local index, which then applies the
same fallback. Both routes reach the same wrong answer.

## Fix

Date the content by the commit that last changed *that blob on that branch*. A
branch with no cell still has the task in its history — it inherited it across
the merge-base — and a walk of `mine` would find `main`'s pre-fork commit and
report Feb 2025, which is exactly right.

Today the walk is only ever asked about branches' *cells*, so a task with no cell
on a branch is never asked about for that branch at all. The fix is to ask.

Note the tension with [[restore-the-last-change-walk-s-e-f70d]]: asking for more
ids widens the walk, which is the thing that task is trying to narrow. Settle the
walk's shape first, then make this read ask it the right question.

A cheaper approximation — fall back to the *default* branch's cell rather than
the newest — is right whenever the default branch has not moved the task since
the merge-base, and wrong when it has. It is strictly better than today, but it
is still a guess about which version is on screen.

## Also

`op-server`'s `PATCH` response echoes the branch just written and dates it with
the same fallback. Its comment claims the headline "already holds this exact
content" — true only when no other branch diverges, which is precisely the case
that breaks.

## Done when

- A read of a task unchanged on the current branch reports the date of the commit
  that produced the bytes it printed.
- The `PATCH` echo dates the version it echoes.
- A test covers the three-branch shape above, and fails against the fallback.
