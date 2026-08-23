---
status: todo
created: 2026-07-31T14:08:22Z
parent: ./00012-tags-registered-labels-name-colo.md
dependencies:
- ./00072-op-store-tag-crud-strict-set-ass.md
---
# Tags wire surface: op-api DTOs, /api/tags routes, TagsChanged

Phase 3 of [[./00012-tags-registered-labels-name-colo.md]]: the wire surface — DTOs in `op-api`, the `/api/tags` routes and task-DTO plumbing in `op-server`.

## Scope
- `op-api`: `TagView { name, display, color, description }`, `CreateTag`, `TagPatch` (color / description / rename), with `Color` as a closed `ToSchema` enum so the generated web client carries the palette names. Extend the task DTOs: `FrontmatterFields` + `Metadata::{from_partial, from_frontmatter, problems}` (`crates/op-api/src/lib.rs:229`, `:259`, `:316`, `:386`), `CreateTask` (`:496`), and `TaskPatch` following the `dependencies` pattern (`tags: Option<Vec<String>>`, `:572`, `apply` at `:588`). New `ChangeEvent::TagsChanged` (`:941`).
- `op-server`: the five routes (`GET/POST /api/tags`, `GET/PATCH/DELETE /api/tags/:name`) as `.routes(routes!(…))` lines in `documented()` (`crates/op-server/src/lib.rs:140-147`), handlers on the `spawn_blocking` + `write_target` pattern (`:627`) writing the live worktree's registry. Error mapping in `From<StoreError> for ApiError` (`:380`): `TagNotFound` → 404, `InvalidColor` → 400, `TagExists` → 409, `TagReferenced` → 409 (`DELETE ?force=` overrides). Publish `TagsChanged` after every tag write via `publish` (`:319`), like the task handlers do (`:543`, `:728`, `:773`); task routes now carry `tags`.

## Verify
`crates/op-server/tests/http.rs` oneshot tests on the existing `store_state`/`send` fixture (`:21`, `:35`): tag CRUD round-trip; duplicate create → 409; bad color → 400; referenced delete → 409, then success with `?force=`; rename via PATCH rewrites referencing tasks; task create/patch with tags validates strict-set; `/api/events` receives `tags_changed` after a tag write.
