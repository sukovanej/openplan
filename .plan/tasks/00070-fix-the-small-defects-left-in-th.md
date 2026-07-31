---
status: in_review
created: 2026-07-31T12:19:40Z
parent: ./00059-web-ui-extract-reusable-componen.md
---
# Fix the small defects left in the ui primitives

Small defects left in the extracted primitives, one pass:

- `Combobox` hardcodes `id="combobox-list"`. Duplicate DOM ids are reachable: `p` (parent picker) and `a` (subtask picker) are independent states in the detail route and can both be open. Use `useId`.
- `Dialog`'s focus trap (`FOCUSABLE`) matches `button, [href], [tabindex]` but not `input`, `select`, `textarea` — harmless for the help overlay, but the first form dialog will skip its fields.
- `statusOrder` is exported from `task-ui` and used nowhere; the board arrives grouped from the server. Delete it.
- `FuzzyText` lives in `combobox.tsx` but is an independent renderer paired with `fuzzy.ts`; give it its own file.
