---
status: in_review
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

## Comments

### 2026-08-24T17:41:01Z by Milan Suk via claude-code

> The scope named op-lint only. Three supporting changes went outside it:
>
> - `op_task::tag::parse_partial` (a new `PartialTag`) — the lenient tag-file parser the linter needs, put with the format it reads rather than duplicated in the snapshot. It types `color` as `FieldResult<Color>`, which is what separates a missing color (fixable) from a bad one (not).
> - `Store::replace_raw_tag` — the tag twin of `replace_raw`, so `--fix` writes materialized colors under the file lock without reflowing the file through `Tag::to_file_string`. It addresses the file, not the registry, so a stem like `Back End.md` is still repairable.
> - `openplan lint <path>` now resolves a `.plan/tags/*.md` target. Without it a pre-commit hook that passes changed paths would stop with "no task file matches".
>
> Choices the task left open:
>
> - Tag files reuse `Code::Frontmatter` and `Code::Title` rather than getting their own codes. The defects are the same ones, and only the message differs. New codes are `tags`, `tag-name`, and `tag-color`.
> - A `color:` outside the palette is reported but never rewritten — there is no one right color to pick. Only an absent or empty `color:` is fixable.
> - The `tags:` repair replaces the whole field in one splice, produced by serde_yaml so it matches what the store writes. One entry that normalizes to nothing therefore makes the whole field unfixable; the other entries are reported but left alone.
> - A non-normalized filename is reported with the name it should take, but is not fixable: a rename moves a file, and `fix` only splices bytes inside one.
