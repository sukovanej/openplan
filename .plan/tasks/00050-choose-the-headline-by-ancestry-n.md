---
status: done
created: 2026-07-29T10:41:31Z
---
# Choose the headline by ancestry, not by a timestamp

## Problem

When a task exists on several branches, the aggregated views show one of them —
the headline — chosen by `Index::recency_of`: the newest timestamp wins. Neither
timestamp git offers answers the question being asked.

**Author time** (what we use) survives a rebase, so a branch that was rebased
onto another still carries the dates it was originally written with. Edit a task
on `feat` Monday, edit it on `main` Wednesday, rebase `feat` onto `main` Thursday
and resolve the conflict by hand: `feat` now contains main's Wednesday edit plus
Thursday's resolution, and still dates as Monday. `main` headlines, showing the
version the resolution superseded.

**Commit time** was tried and is worse. A rebase rewrites the commit date of
every commit it replays, so the most recently rebased branch looks like the
newest work on every task it carries — including tasks it only inherited. This
repo hit it immediately: `task-timestamps` had backfilled `created` into a task
it otherwise never touched, and one rebase made that branch outrank
`daemon-write-authority`'s genuine `in_review`, so the board reported `backlog`.

```
daemon-write-authority   author=2026-07-28  commit=2026-07-28   in_review
task-timestamps          author=2026-07-16  commit=2026-07-29   backlog
                                            ^ the rebase, not the edit
```

The two failures are the same failure: a timestamp describes when a commit was
written or replayed, and the question is which version *supersedes* the other.

## Direction

Ask git what it actually knows: containment. If one branch's version of a task
descends from another's — the commit that produced it has the other's commit as
an ancestor — that version is strictly later, whatever either clock says. Both
scenarios above resolve correctly under that rule, and neither needs a date.

Timestamps stay as the tie-break for versions with no ancestry relation, which is
the only case where "newest" is genuinely a judgement call. Author time is the
better tie-break there: it describes when the work was done rather than when it
was last replayed.

Cost to weigh: an ancestry query per contested pair, against a graph walk we
already do for merge-bases. Only tasks living on more than one branch with
differing blobs need it — usually few.

## Done when

- A rebased branch headlines over the branch it was rebased onto.
- A branch that merely replays or backfills does not outrank newer work
  elsewhere — covered today by
  `a_replayed_branch_does_not_outrank_newer_work_elsewhere`.
- Both are tests, and both fail against a timestamp-only rule.
