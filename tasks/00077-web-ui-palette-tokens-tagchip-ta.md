---
status: backlog
created: 2026-07-31T14:08:22Z
parent: ./00012-tags-registered-labels-name-colo.md
dependencies:
- ./00073-tags-wire-surface-op-api-dtos-ap.md
---
# Web UI: palette tokens, TagChip, tag rendering, live registry

Phase 7 of [[./00012-tags-registered-labels-name-colo.md]]: the read side of tags in the web UI — palette tokens, the chip, rows and detail, live registry updates. No editing yet.

## Scope
- Regenerate the client (`mise run generate-web-client`) — brings `Color`, `TagView`, the tag routes, and `tags` on the task DTOs.
- `@open-planner/ui`: palette CSS variables in `styles.css` — light `:root` + `.dark` pairs per color plus the `@theme inline` mappings, mirroring the status triples (`web/packages/ui/src/styles.css:30-47`). Tailwind purges dynamic class names, so the chip needs a static `Record<Color, string>` class map (the `BranchTag` pattern, `web/packages/task-ui/src/branch-tag.tsx:6-12`).
- `@open-planner/task-ui`: `TagChip` built on `Tag` (`web/packages/ui/src/tag.tsx`); the dangling variant is gray with a warning icon and a `Tooltip` explaining the tag doesn't exist on this branch and that any tags edit drops it. `metadata.ts` accessors + `problems` (`web/packages/task-ui/src/metadata.ts:38-50`) learn `tags`. Barrels updated in both packages.
- App: a `tagsQuery` in `lib/store.ts` over `GET /api/tags`; `tags_changed` added to the hand-written `ChangeEvent` mirror and `applyChange` (`web/packages/app/src/lib/events.ts:3-62`) → refetch the registry; chips on task rows (`web/packages/app/src/routes/list.tsx:134-234`) and the detail header (`web/packages/app/src/routes/detail.tsx:76-125`), name → color resolved via the registry, unknown → dangling chip.

## Verify
Vitest: `TagChip` palette + dangling variants (happy-dom, task-ui); `events.test.ts` covers `tags_changed`; row/detail render with tags. `pnpm lint`, `test`, `build` green in light + dark.
