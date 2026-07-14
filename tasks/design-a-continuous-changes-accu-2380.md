---
status: backlog
---
# Design a continuous-changes accumulation branch

A design task. Produce a SPEC section (extending §7) and an implementation plan;
no code yet.

## Problem

The spec's invariant is **reads global, writes local** (§7.1): a task can only be
mutated on the branch whose worktree is checked out. That fits *code-coupled*
edits — an agent on `feat/auth` flipping a task to `in_progress` as part of its
loop (§7.8), where `status` is correctly a branch fact (§7.2).

It leaves a gap for **ambient / triage edits that belong to no feature branch**:
reprioritizing, rewriting a description, adding a subtask, changing `rank`, or any
edit made through the UI. Today these must land on whatever branch happens to be
checked out, which is arbitrary. We want a dedicated lane where such changes
**accumulate over time** and are then published into `main`.

## Proposed model

A dedicated **updates ref** owned by the daemon, the write target for ambient/UI
edits, distinct from the code-coupled feature-branch lane.

**Storage — custom ref, not a branch.** Lives at **`refs/open-plan/rolling-updates`** (a
custom namespace, *not* under `refs/heads/*`). Consequences: excluded from the
default push refspec (can't be accidentally shared), invisible to `git branch` /
`git checkout` / the feature-branch swimlane, addressed by the daemon by full ref
path. It is still *explicitly* pushable for backup (below).

**Accumulation is worktree-less.** No standing worktree is kept for the ref. An
ambient edit is committed straight into the object DB: write blob → build tree →
create commit (parent = current tip) → compare-and-swap the ref. The daemon is
the **sole writer**, so it serializes writes in-process — no cross-process file
lock needed. Inbound edits are **debounced/coalesced** into few commits (rapid
UI keystrokes / drag-reorder collapse together), keeping history legible.

The **section-aware merge driver** (§7.7) is the enabler for reconciliation:
non-overlapping section/field edits auto-merge, so the flow is mostly plumbing +
policy, not a new merge algorithm.

**Flow — rebase to refresh, fast-forward to publish:**
- `main → updates` **refresh** — **rebase** `refs/open-plan/rolling-updates` onto `main`, so
  the ref is always `main` + a clean linear stack of pending ambient commits
  (rebase is safe: the ref is private and never consumed by others; see backup).
  **Event-driven** off the ref-watcher (§7.5) — triggered when `main`'s ref
  moves, not on a timer — and **debounced** on a quiet window (default ~1 min):
  further ref moves reset the timer; the rebase runs once `main` has settled,
  coalescing a burst into one refresh. The merge driver fires during the replay,
  so non-overlapping edits reconcile automatically; a genuine same-section
  conflict stops the rebase → mark that task `conflicted` in the UI (§7.7),
  `git rebase --abort` to keep the ref usable, hold, and retry on the next
  refresh once a human reconciles. All refresh work belongs to the **daemon**; if
  it is down nothing refreshes (acceptable — refresh isn't urgent, publish is
  manual). The **periodic sweep** and a **startup reconcile** are in-daemon safety
  nets (reconcile the ref's merge-base vs. current `main`) covering `notify`
  events dropped under load or missed while the daemon was off — **not** a
  daemon-down path.
- `updates → main` **publish** — **Manual / explicit only**, and a pure
  **fast-forward**: because the ref is always rebased on top of `main`, publishing
  just advances `main` to the ref's tip. No merge commit on `main`, and publish
  **can never conflict** — all conflicts are forced to surface earlier, at refresh
  time. No background process ever mutates `main`.

**Merges need a worktree.** Both rebase and (the FF is trivial, but any real
merge) require a checkout, so the daemon spins up an **ephemeral worktree** only
at refresh time, runs the rebase there (driver fires), updates the ref, then
tears it down. Between refreshes there is no worktree on disk.

**Backup sync (decided: durability only).** The daemon **force-pushes**
`refs/open-plan/rolling-updates` to a configured backup remote so un-published ambient edits
survive disk/machine loss. Safe because this machine is the **sole writer** and
the remote is a write-only mirror nobody pulls — force is fine, and there is no
distributed concurrency. Multi-machine / collaborative sync is **out of scope**
(it would forbid rebase, require merge-based flow + push-race handling, and cross
the §7.10 single-machine boundary).

## Decisions made

- **Storage:** `refs/open-plan/rolling-updates`, a custom ref (not a branch) — local by
  default, addressed by the daemon; accumulation is worktree-less (blob → tree →
  commit → ref CAS), daemon is the sole serialized writer.
- **Refresh (`main → updates`):** **rebase** the ref onto `main` (not merge) so it
  stays linear `main` + pending edits; **event-driven** off the ref-watcher (§7.5)
  and **debounced** (~1 min). Periodic sweep + startup reconcile are in-daemon
  safety nets for dropped/missed watcher events — **not** a daemon-down path.
- **Publish (`updates → main`):** **manual, fast-forward only** — cannot conflict
  (conflicts surface at refresh). `main` is never mutated automatically.
- **Merge machinery:** an **ephemeral worktree** is created only for the refresh
  rebase and torn down after; none stands between refreshes.
- **Backup:** force-push `refs/open-plan/rolling-updates` to a mirror remote for durability;
  sole-writer, no distributed concurrency. Multi-machine/collaborative sync out of
  scope.
- **Debounce (two places):** inbound ambient edits coalesce into few commits; the
  refresh coalesces a burst of ref moves into one rebase.
- **Routing — by feature-context, one rule:** a write lands on the current
  *feature branch* if it has one; otherwise it is *ambient* and lands on
  `refs/open-plan/rolling-updates`. "Ambient" = a write with no feature-branch
  context. Concretely:
  - CLI/agent in a **feature** worktree (`feat/*`) → that feature branch
    (unchanged, §7.1 writes-local).
  - CLI/agent in the **`main`** (trunk) worktree → `rolling-updates`. This also
    enforces CLAUDE.md's "never write to the main checkout" — the write is
    rerouted rather than allowed to dirty `main`.
  - **UI**, global task-centric view (no worktree context) → `rolling-updates`.
  - **UI**, worktree swimlane scoped to a branch → that branch (still a feature
    context; consistent with §9).
  - Escape hatch: a `--ambient` flag forces a CLI write to `rolling-updates` even
    from a feature worktree (for triage edits unrelated to the current branch).
- **UI surface — one header control + review popover (Option C):** a single
  color-coded sync icon in the header (beside daemon status / theme toggle) with a
  pending-count pill. State → color + tooltip, reusing the app's existing
  convention (`emerald`=good, `amber`=warn from `connection-status`), adding blue
  for "action available":
  - *In sync* — muted/gray, check icon: "nothing to publish."
  - *Pending (N)* — blue, count pill: "N changes ready to publish to main."
  - *Syncing* — blue spinner: publish or auto-rebase running.
  - *Blocked* — amber, warning icon: a refresh conflict paused sync.
  - *Offline* — dim, disabled: daemon down.

  **Click opens a "Rolling updates" review popover** (does *not* publish blind):
  it lists the pending ambient changes (task + what changed), flags conflicting
  ones in amber, and carries the primary **"Publish N to main"** action inside —
  so publish stays the manual, explicit, reviewable step. In the *blocked* state
  the popover shows the conflict and a "Resolve conflict" action instead of
  Publish. This is where the conflict-hold state is surfaced and resolved.
  Interactive mockup of all states/layouts:
  [../assets/sync-button-options.html](../assets/sync-button-options.html).
- **v1 publish target is `main` only.** Fan-out to active feature branches is v2.

## Open questions

- **Merge driver under rebase:** confirm the custom §7.7 driver actually fires
  during a `git rebase` replay (expected, since rebase uses the merge machinery),
  and whether a worktree-less path (`git merge-tree --write-tree` / gitoxide)
  could invoke it and drop the ephemeral worktree entirely. Verify in the spike.
- **Presence & claims** (§7.6) — the cross-branch collision-warning question for
  ambient edits is tracked on `presence-show-who-s-working-on-w-7185`, not here.
- **v2 fan-out:** publishing ambient edits to active feature branches, not just
  `main`.

## Deliverable

A redline to SPEC.md (new subsection under §7) covering the branch home, the
bidirectional flow, the conflict-gated manual publish, routing rules, and the UI
surface — plus a phased implementation plan.
