---
status: done
created: 2026-07-31T14:08:22Z
parent: ./00012-tags-registered-labels-name-colo.md
---
# op-task: Color palette, tag-name normalization, tags frontmatter field

Phase 1 of [[./00012-tags-registered-labels-name-colo.md]]: the tag domain model in `op-task`, with no store or wire surface yet.

## Scope
- `Color` palette enum (~12 named colors) modeled on `Status` (`crates/op-task/src/lib.rs:19-67`): `#[serde(rename_all = "snake_case")]`, `ToSchema`, `ALL`, `as_str`, `FromStr` with a parse error listing the valid names. Deterministic default color: an explicit stable hash of the normalized name (e.g. FNV-1a spelled out in code — not `DefaultHasher`, whose output may change across Rust releases) → palette index.
- Tag name normalization modeled on `slug` (`crates/op-task/src/lib.rs:195`) but *rejecting* instead of dropping: lowercase, spaces/underscores → hyphens, collapse runs; if the result still fails `[a-z0-9][a-z0-9-]*`, error with the rule in the message. `"Front End"` → `front-end`; `"C++"` → error.
- Tag file model (frontmatter `color: Color` + single `# H1` display name + free-markdown description body) reusing the task file machinery: `split_frontmatter` (`crates/op-task/src/lib.rs:543`), byte-for-byte body preservation. A missing `color:` on read falls back to the derived default (readers never hard-fail); expose whether the color was materialized so lint can flag the omission.
- `tags` on the task model: `Frontmatter` gains `tags: Vec<String>` (`skip_serializing_if` empty, sorted + deduped on write — unlike `dependencies`, which stays order-preserving), plus the lenient path: `PartialFrontmatter` + `extract_fields` (`crates/op-task/src/lib.rs:434`, `:488`). `tags:` currently survives invisibly via `Frontmatter::extra` (`:89-90`) — moving it to a real field must not break roundtrip of existing files.

## Verify
`crates/op-task/tests/`: normalization accept/reject table; color parse + deterministic default (same name → same color); tag file roundtrip (body untouched, display case preserved); task frontmatter `tags` sorted/deduped/omitted-when-empty with the body byte-for-byte across a tags write.
