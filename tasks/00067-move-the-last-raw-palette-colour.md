---
status: done
created: 2026-07-31T12:19:40Z
parent: ./00059-web-ui-extract-reusable-componen.md
---
# Move the last raw palette colours onto tokens

The extraction's token constraint — no component ships a bare `red-500` — has three stragglers:

- `task-ui/src/branch-tag.tsx` colours change kinds with raw `emerald-600` / `sky-600` / `rose-600`, and uses those light-calibrated hues in dark mode too. Add change-kind tokens next to the status tokens in `ui/src/styles.css` (`--change-added`, `--change-modified`, `--change-deleted`, with dark overrides) and put `BranchTag` on them.
- `app/src/components/connection-status.tsx` ships `text-emerald-600` / `text-amber-600`. The first is the existing `success` token; the second wants a new `warning` token.
- `--danger-border` has no dark override, so the danger toast border and the `danger` button hover wear the light hue in dark mode. Every sibling danger token has a dark value.
