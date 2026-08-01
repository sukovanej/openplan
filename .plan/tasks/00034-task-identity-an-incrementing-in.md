---
status: done
created: 2026-07-16T18:50:25Z
dependencies:
- ./00033-daemon-as-single-write-id-author.md
---
# Task identity: an incrementing integer

Task identity was a **random** id: `slug(title)` plus a 2-byte random hex suffix,
e.g. `ship-login-page-3d0c`. It becomes a **monotonically incrementing integer**
and nothing else, e.g. `42`. The file keeps a readable name —
`00042-ship-login-page.md` — but the padding and the slug are decoration outside
the id: the task is found by the number its file name starts with.

## Motivation

Sequential ids (`42`) are shorter, memorable, orderable, and pleasant to type and
reference (`[[./00042-publish-fast-forward-main-to-the.md]]`) versus opaque random hex.

## Allocation model (settled — single-machine)

open-plan is single-machine, so [[./00033-daemon-as-single-write-id-author.md]] makes
the machine daemon the sole writer. Allocation follows from that:

- `next = max(numeric id over all local branches) + 1`, computed from the set the
  matrix builder already aggregates (working-tree copies for checked-out
  branches + committed blobs for the rest). No stored or committed counter.
- The single daemon serializes creates and holds an in-memory reservation set, so
  the floor accounts for all branches and back-to-back creates never repeat.
- Because the number is issued exactly once globally, two *different* tasks can
  never share an id; a merge only ever sees one task `N` with divergent versions.
  No merge-time renumbering or collision-repair is needed.

This task depends on the daemon-authority work landing first — do not reintroduce
a per-branch `op-store` counter here; allocation belongs behind the daemon.

## Scope

- Id format: `<n>`. Drop `rand_hex(2)` outright. Define the canonical id grammar —
  decimal, no sign, no padding — and reject anything else as an id.
- File name: `<nnnnn>-<slug(title)>.md`, zero-padded so a listing sorts in task
  order. A file is resolved by its numeric prefix, so re-slugging it by hand
  keeps the task.
- Allocation source lives behind the daemon (see the dependency), not in a
  per-branch `op-store` high-water mark.
- Migration: rename existing `.plan/tasks/*.md` and rewrite every reference to old
  ids — `parent`, `deps` (including `id#Section`), and `[[id]]` body refs — as a
  one-shot pass, including tasks that live on other local branches.
- Aggregation (`op-index` / matrix builder) and presence (keyed by logical id)
  must agree on the new identity.

## Acceptance

- New tasks get an incrementing numeric identity; `oplan create` prints it.
- No two tasks ever share a filename — single-writer allocation over a
  global-across-branch floor.
- Existing tasks and all their references migrate without dangling links.
- `cargo build && cargo test && cargo fmt --check && cargo clippy -- -D warnings` pass.
