---
status: backlog
created: 2026-07-30T18:53:07Z
parent: ./00059-web-ui-extract-reusable-componen.md
dependencies:
- ./00060-scaffold-open-planner-ui-and-tas.md
---
# Panel, Row and Section primitives; rewire the list and detail routes

The framed container and the row are spelled once per route, and the
current-row treatment three times. Give them one implementation and rewire both
routes onto it. This is the step that can move pixels, so it carries the
interactive check.

## `Panel` / `PanelHeader` / `PanelBody`

From `routes/list.tsx:57-61` and `routes/detail.tsx:73-86`: a column that fills
its height, clips, and draws its outline as an inset ring rather than a border —
so a selected row's own ring lands on the same pixels instead of doubling up.
`PanelHeader` is the fixed `h-11` uppercase strip and takes arbitrary children
(the list passes a label, the detail passes status, id, title and the parent
control). `PanelBody` is the `min-h-0 flex-1 overflow-y-auto` region.

## `Row`

One row with `active`, `hoverable` and `last` props, replacing:

- `CURRENT_ROW` / `HOVERED_ROW` in `routes/list.tsx:119-125`,
- the subtask `<li>` styling in `routes/detail.tsx:334-337`,
- the combobox option styling in `components/search-combobox.tsx:154-157`.

Keep the mechanism the list already found: the current row drops its own bottom
border and draws the outline as an absolutely-positioned `after:` overlay offset
`-1px` top and bottom, so the row's height does not change with selection.
Hover repeats the same classes literally — Tailwind only emits what it can read
in the source. Row height stays an integer number of pixels; a fractional height
lands borders on half device pixels and the row shimmers on Retina when it is
selected.

`Row` owns appearance only. The pointer/keyboard arbitration stays with the
callers: `hoveredRow` and `rowCursor` in `src/lib`, and the rule that the pointer
only claims a row while the keyboard cursor is idle.

## `Section`

From the subtasks header in `routes/detail.tsx:289-305`: top rule, uppercase
heading, optional `CountPill`, and a right-aligned action slot.

## Rewire

`routes/list.tsx` and `routes/detail.tsx` use the three components. `TaskRow`,
`HeaderRow` and `SubtasksSection` keep their data handling and lose their
markup.

## Verify

Package render tests for `Row` covering active, hoverable and last. Then, over
the dev server with headless Chrome per the repo's web-UI verify recipe:

- the list and detail frames are visually identical to `main`, side by side;
- `j` / `k` walk rows with no height change and no border shimmer on a selected
  row;
- moving the pointer hands the current row back to it, and only over a row;
- the last row in the last group still drops its divider;
- the subtask list and the combobox options highlight as before.
