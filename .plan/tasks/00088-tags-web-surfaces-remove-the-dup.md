---
status: done
created: 2026-08-26T07:12:36Z
parent: ./00012-tags-registered-labels-name-colo.md
---
# Tags web surfaces: remove the duplicated query map, form, and popover code

The tags work left three copies of one mechanism each. A later fix has to reach
every copy.

## Do this

- `web/packages/app/src/lib/store.ts` holds three get-or-insert query maps:
  `keyed`, `taskQuery`, and `tagsQuery`. Only `taskQuery` bounds its map and
  prunes unmounted entries. Generalize `keyed` to take a key builder and an
  optional bound. Use it for all three.
- `NewTag` and `TagForm` in `web/packages/app/src/routes/tags.tsx` are the same
  form. Both hold a name, a description, and a writing flag. Both trim. Both
  submit through `runTagMutation`. They differ only in create against patch, and
  in whether they clear or close. They already drifted once: the empty-name guard
  reached one of them a release later than the other. Make them one component.
- The dismiss-on-outside-click effect in `Palette` is a copy of the one in
  `Combobox`. Put it in a shared hook.

## Acceptance criteria

- [ ] One get-or-insert helper serves every keyed query in `store.ts`.
- [x] One form component serves both tag create and tag edit.
- [x] One hook serves both dismiss-on-outside-click sites.
- [x] `pnpm lint`, `typecheck`, `test`, and `build` pass.

## Comments

### 2026-09-05T10:04:54Z by Milan Suk via claude-code

> The first criterion is moot: OPP-89 replaced the custom store with TanStack Query, so `store.ts` and its `keyed`, `taskQuery`, and `tagsQuery` maps no longer exist. Nothing remains to unify.
