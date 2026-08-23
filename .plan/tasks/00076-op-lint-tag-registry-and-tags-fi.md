---
status: todo
created: 2026-07-31T14:08:22Z
parent: ./00012-tags-registered-labels-name-colo.md
dependencies:
- ./00072-op-store-tag-crud-strict-set-ass.md
---
# op-lint: tag registry and tags-field lints

Phase 6 of [[./00012-tags-registered-labels-name-colo.md]]: lint coverage for the tag registry and the `tags:` field.

## Scope
- `Snapshot::from_store` (`crates/op-lint/src/snapshot.rs:22`) additionally scans `.plan/tags/*.md` into tag entries (name, source, parsed color/display), with a `from_files`-style test constructor to match (`:50`).
- New `Code` variants (`crates/op-lint/src/diagnostic.rs:29-60` — the enum and `as_str` both), new rules appended to the const slices (`crates/op-lint/src/rules.rs:32-44`):
  - a tag filename is a normalized name; the file has a single H1.
  - `color:` present and in the palette; an omitted color is flagged and fixable — materialize the derived default through the fix pipeline (`crates/op-lint/src/fix.rs`).
  - a task's `tags:` entries are shape-valid normalized names, sorted and deduped (fixable).
- Deliberately **no** dangling-reference lint: a name unknown on this branch is legal by design ([[./00012-tags-registered-labels-name-colo.md]] referential integrity).

## Verify
`crates/op-lint/tests/rules.rs` with in-memory fixtures; fix tests for color materialization and set normalization in `tests/fix.rs`.
