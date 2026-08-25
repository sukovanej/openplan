---
status: in_review
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

## Comments

### 2026-08-25T12:30:47Z by Milan Suk via claude-code

> The tag-management surface became a route, `/:project/tags`, not a dialog. The
> registry is per project, and a route carries the project in the URL. The board
> header links to it. The route also clears the board's row cursor, because `j`
> and Enter would otherwise open a task the reader cannot see.

### 2026-08-25T12:30:47Z by Milan Suk via claude-code

> The picker also registers a tag, which the task gave to the management surface
> alone. A query the registry does not spell offers "Register “X” as a new tag";
> it POSTs the tag, then PATCHes the task with the name the registry answered
> with. That keeps the normalization rule in the store: the web never spells a
> name of its own.

### 2026-08-25T12:30:47Z by Milan Suk via claude-code

> `runMutation` now resolves `true` or `false` instead of `void`. The delete row
> sends no `force` on the first attempt, and the daemon's refusal is what earns
> the explicit "Delete anyway" button — so the row has to know the write was
> refused. The reason stays in the mutation-error toast.

### 2026-08-25T12:30:47Z by Milan Suk via claude-code

> The 409 the toast shows for a referenced delete is `StoreError::TagReferenced`,
> which reads "pass --force to delete it". A CLI flag in a web toast reads wrong.
> The fix belongs in `op-store` (a neutral message) plus an `op-cli` hint, and
> this task is web-only, so it is left alone. The row's "Delete anyway" button
> carries the consequence in its tooltip.

### 2026-08-25T12:30:47Z by Milan Suk via claude-code

> Verified by hand against a throwaway repo, not this one: the daemon writes to
> the project root, and the primary checkout must take no writes. A scratch
> `OPENPLAN_HOME` daemon on port 7399 served a temporary repo, and headless Chrome
> drove create, assign, prune-danglings, recolour, rename, referenced delete, and
> force delete in light and dark. A read-only branch (no writable worktree) was
> not reachable in that setup, so the `read-only` marker on the tags row is
> covered by inspection alone.
