---
status: backlog
created: 2026-07-28T08:19:15Z
---
# Web UI: row tooltips are unreachable under the title stretched-link

A task row's title link is a stretched link — `after:absolute after:inset-0` over the
whole `relative` row — so it, not the cell under the pointer, receives every hover in
the row. Any `title` attribute on another cell is therefore dead: `elementFromPoint` at
the centre of the cell returns the `<a>`.

Affected today: the relative-time cell (whose tooltip carries the exact instant, the
only place the precise timestamp is reachable in the list) and the branch badges. The
sibling "Subtask of <parent>" link already works around this with `relative z-10`.

## Why it is not a one-line fix

Adding `relative z-10` to a cell lifts it out of the overlay and restores its tooltip,
but that area of the row then no longer navigates on click — the stretched link is what
makes the whole row clickable. So the fix trades a working tooltip for a dead click
target, cell by cell.

Worth deciding once, for the row as a whole:

- Keep the stretched link and accept that rows carry no tooltips, dropping the `title`
  attributes that currently promise one.
- Raise the metadata cells (`relative z-10`) and accept that clicks there only move the
  row cursor, without navigating.
- Replace the stretched link with an explicit row-level click handler that navigates, so
  cells keep both their tooltips and click-through.

## Refs

- `web/packages/app/src/routes/list.tsx` — `TaskRow`; the title `Link`'s
  `after:absolute after:inset-0`, and the `relative z-10` already on the parent link.
- `web/packages/app/src/components/time-ago.tsx` — the `title` that never fires here.
- `web/packages/app/src/components/branch-tag.tsx` — same, pre-existing.

## Verify

Hover the age and a branch badge in a list row: both should show their tooltip, and
whatever click behaviour the chosen option specifies should hold for the same spots.
