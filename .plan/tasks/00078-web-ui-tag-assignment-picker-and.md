---
status: todo
created: 2026-07-31T14:08:22Z
parent: ./00012-tags-registered-labels-name-colo.md
dependencies:
- ./00077-web-ui-palette-tokens-tagchip-ta.md
---
# Web UI: tag assignment picker and tag management surface

Phase 8 of [[./00012-tags-registered-labels-name-colo.md]]: editing — assign/unassign on a task, and the tag-management surface.

## Scope
- Assign/unassign on the detail route: a picker on the `ParentPicker` / `Combobox` pattern (`web/packages/app/src/routes/detail.tsx:233`), listing the registry, mutating via `runMutation(patchTask(id, { tags }))` (`:255`). Validation is strict whole-set, so every PATCH sends the full set **pruned of dangling names** — the dangling chip's tooltip forewarns that drop; its only affordance is remove.
- Tag management: a surface over `POST/PATCH/DELETE /api/tags` — create (name + optional description), recolor via a palette picker over the fixed named set, rename, and delete with the 409-referenced flow surfaced (force offered explicitly, never defaulted).
- Errors surface through the existing mutation error path (`mutationError`, `web/packages/app/src/lib/store.ts:179`).
- No filtering: chips stay inert beyond tooltip/remove ([[./00012-tags-registered-labels-name-colo.md]] keeps filtering a follow-up).

## Verify
Vitest coverage for the picker's prune-danglings behavior and the palette picker. End-to-end by hand per the web-ui verify recipe: create → assign → recolor → rename → referenced delete → force delete, chips updating live in light + dark. `pnpm lint`, `test`, `build` green.
