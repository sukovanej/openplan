---
status: todo
created: 2026-07-31T14:08:22Z
parent: ./00012-tags-registered-labels-name-colo.md
dependencies:
- ./00071-op-task-color-palette-tag-name-n.md
---
# op-store: tag CRUD, strict-set assignment validation, rename/delete integrity

Phase 2 of [[./00012-tags-registered-labels-name-colo.md]]: tag storage and referential integrity in `op-store`.

## Scope
- `tags_dir()` → `.plan/tags` beside `tasks_dir()` (`crates/op-store/src/lib.rs:125-129`); tag file enumeration by normalized-name stem.
- CRUD: `create_tag` / `read_tag` / `write_tag` / `rename_tag` / `delete_tag` / `list_tags` / `tag_exists`. Reuse the task write discipline — `with_lock`'s open-then-`same_inode` retry (`crates/op-store/src/lib.rs:307`, `:550`) and `atomic_replace`/`write_temp` (`:469`) — generalized to the tags dir (today's helpers assume `tasks_dir`). Create publishes via non-clobbering `hard_link` like `link_id` (`:252`) so a duplicate name errors instead of overwriting.
- New `StoreError` variants (`crates/op-store/src/lib.rs:39-64`): `TagNotFound`, `TagExists`, `TagReferenced { count }`, `InvalidColor`.
- Strict whole-set assignment validation in `Store::validate` (`crates/op-store/src/lib.rs:385-439`): every name in a written `tags` set must exist in this store's registry — deliberately *unlike* dependencies, which skip refs already present in `old` (`:421`). The error hints `openplan tag create`.
- `rename_tag(old, new)`: refuse when `new` exists (`TagExists`); rename the file, then rewrite `tags:` in every referencing task on this branch, each under its own task lock, frontmatter-only.
- `delete_tag(name, force)`: scan the branch's task files for references; refuse with `TagReferenced { count }` unless forced.

## Verify
`crates/op-store/tests/store.rs`: tag CRUD roundtrip; duplicate create errors; assignment rejects an unknown name even when the task already carried it (strict set) and accepts once it is dropped; rename rewrites referencing tasks and refuses an existing target; delete honors the reference scan and force; concurrent writes to one tag file serialize.
