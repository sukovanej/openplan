---
status: backlog
deps:
- daemon-as-single-write-id-author-c1e9
---
# Task identity: incrementing integer id prefix instead of the random 4-char suffix

Task identity is currently a **random** id: `slug(title)` plus a 2-byte random hex
suffix, e.g. `ship-login-page-3d0c` (`op-store` `link_new_id` → `rand_hex(2)`).
Replace that random suffix with a **monotonically incrementing integer prefix**
that becomes the task's identity, e.g. `42-ship-login-page`. The number is the
stable identity; the slug tail is human-readable decoration only.

## Motivation

Sequential ids (`42`) are shorter, memorable, orderable, and pleasant to type and
reference (`[[42]]`) versus opaque random hex.

## Allocation model (settled — single-machine)

open-plan is single-machine, so [[daemon-as-single-write-id-author-c1e9]] makes
the machine daemon the sole writer. Allocation follows from that:

- `next = max(numeric id over all local branches) + 1`, computed from the set the
  §7.4 matrix builder already aggregates (working-tree copies for checked-out
  branches + committed blobs for the rest). No stored or committed counter.
- The single daemon serializes creates and holds an in-memory reservation set, so
  the floor accounts for all branches and back-to-back creates never repeat.
- Because the number is issued exactly once globally, two *different* tasks can
  never share an id; a merge only ever sees one task `N` with divergent versions.
  No merge-time renumbering or collision-repair is needed.

This task depends on the daemon-authority work landing first — do not reintroduce
a per-branch `op-store` counter here; allocation belongs behind the daemon.

## Scope

- Id format: `<n>-<slug>`. Keep `slug()`; replace only the identity component
  (drop `rand_hex(2)` from `link_new_id`). Define the canonical id grammar and
  reject/normalize malformed ids.
- Allocation source lives behind the daemon (see the dependency), not in a
  per-branch `op-store` high-water mark.
- Migration: rename existing `.plan/tasks/*.md` and rewrite every reference to old
  ids — `parent`, `deps` (including `id#Section`), and `[[id]]` body refs — as a
  one-shot pass, including tasks that live on other local branches.
- Aggregation (`op-index` / matrix builder, §7.4) and presence (§7.5, keyed by
  logical id) must agree on the new identity.
- SPEC updates: §3.1 (Identity) and §7.1 (many-branch model) rewritten to describe
  the numeric identity and how single-writer allocation keeps it collision-free.

## Acceptance

- New tasks get an incrementing numeric identity; `oplan create` prints it.
- No two tasks ever share a filename; SPEC §3.1/§7.1 document exactly why
  (single-writer, global-across-branch floor).
- Existing tasks and all their references migrate without dangling links.
- `cargo build && cargo test && cargo fmt --check && cargo clippy -- -D warnings` pass.
