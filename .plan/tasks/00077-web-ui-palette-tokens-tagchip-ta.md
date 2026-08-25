---
status: in_review
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

## Comments

### 2026-08-25T11:37:51Z by Milan Suk via claude-code

> The client needed no regeneration: OPP-73 already committed `Color`, `TagView`, the tag routes, and `tags` on the task DTOs, and `events.ts` already carried `tags_changed`. Regenerating produced an empty diff. The mise task itself was broken, so this branch fixes it: `pnpm exec` writes a workspace banner to stdout, which the redirect wrote into `src/index.ts`. It now runs `pnpm --silent exec`.

### 2026-08-25T11:37:56Z by Milan Suk via claude-code

> The registry reaches the chips as a prop, not a hook: `TaskTags` takes `ReadonlyMap<name, TagView> | undefined` and the app passes `useTags(project)`. This keeps `@open-planner/task-ui` free of the store — as `BranchBadges` is — and it puts the row and detail rendering under a package with happy-dom, which the app package does not have (node environment, `tests/**/*.test.ts` only). So the Verify line's row/detail render tests live in `task-ui/tests/task-tags.test.tsx` rather than in the app package.

### 2026-08-25T11:38:00Z by Milan Suk via claude-code

> `undefined` for an unread registry is load-bearing. A registry read as an empty map would make every name on every row look dangling for the first frames, so `TaskTags` shows nothing until the read succeeds. A failed read holds the chips back too — the same daemon answers the board, so the page is already in an error state.

### 2026-08-25T11:38:05Z by Milan Suk via claude-code

> `TagChip` overrides `Tag`'s padding and line height (`py-0.5 leading-4`) to make the chip 22px rather than 23.75px. `Row`'s `divided` variant owes its borders a whole number of pixels, and the chips are the tallest thing on a tagged row, so they decide its height. Measured in Chrome at `deviceScaleFactor: 2`: tagged rows come out 67px and 85px, untagged 61px. A branch tag is still 23.75px — that was so before this branch, and changing `Tag` itself would move every branch tag.

### 2026-08-25T11:38:09Z by Milan Suk via claude-code

> Two additions the scope line does not name. A chip shows the registry's `display` name, not the normalized `name`, so `# Backend` reads as `Backend`; a dangling chip shows the raw name, all it has. And a tag with a description gets a `Tooltip` carrying it — otherwise `description` stays unreadable until the tag-management surface lands.

### 2026-08-25T11:38:15Z by Milan Suk via claude-code

> Runtime check no test holds, against an isolated daemon on port 7399 over a scratch repo, driven by headless Chrome over CDP. All twelve palette colors render on a row and on the detail header, in light and dark. A hand-written dangling reference renders muted, dashed, and warned. Live registry updates work without a reload: `openplan tag set blue-tag color pink` recolored the chip, and `openplan tag rename backend platform` renamed it on both rows that carried it — `tags_changed` reaches `tagsQuery` through `refreshVisible`. Note for the next check: an SSE stream through the Vite dev proxy never opens, so the header reads "daemon down" there; point Chrome at the daemon's own port to test realtime.

### 2026-08-25T11:38:19Z by Milan Suk via claude-code

> Left for the assign phase: `TagChip` takes no `onSelect`/`selected`. `Tag` underneath already carries both, so a filter or an edit affordance is a passthrough, not a rewrite. Adding them now would ship an API with no caller.
