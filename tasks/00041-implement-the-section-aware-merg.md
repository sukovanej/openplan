---
status: backlog
created: 2026-07-26T15:40:55Z
parent: ./00039-continuous-changes-accumulation-v.md
dependencies:
- ./00023-design-a-continuous-changes-accu.md
---
# Implement the section-aware merge driver (replace stub)

Replace the `oplan merge-driver` stub with the real 3-way merge.
This is **Phase 0** of the rolling-updates plan
([[./00023-design-a-continuous-changes-accu.md]]) and gates the rest of it: refresh
reconcile relies on the driver auto-merging non-overlapping edits and
conflicting only on genuine same-section overlaps.

## Today

`crates/op-cli/src/mergedriver.rs` is a stub: it parses the file and counts
sections, then **conflicts on any byte difference** (`ours == theirs` else
`Conflict`). No actual merge happens.

## Scope

- Real 3-way merge at **frontmatter-field + section granularity**, reusing
  `op-md` heading/section addressing and `op-task` parsing.
  - Non-overlapping section edits and non-overlapping frontmatter-field edits
    (`status`, `deps`, `rank`, …) **auto-merge**.
  - Only genuine **same-section / same-field** divergence conflicts.
  - The parser must never choke on `<<<<<<<` markers.
- Ship `/.plan/**.md merge=openplan` in a repo `.gitattributes` and register the
  driver in local git config on project init.

## Verify

Tests in `crates/op-cli/tests/`:
- non-overlapping section edits on two sides → merged, exit 0;
- non-overlapping frontmatter fields → merged, exit 0;
- same-section divergence → non-zero exit;
- confirmed worktree-less: `git merge-tree --write-tree` invokes the driver and
  returns the merged tree (clean) or the stage-1/2/3 conflict set (see the spike
  in [[./00023-design-a-continuous-changes-accu.md]]).
