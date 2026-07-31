---
status: done
created: 2026-07-31T11:21:51Z
parent: ./00059-web-ui-extract-reusable-componen.md
---
# Web UI: Tooltip primitive; tooltips for status icons and times

A `Tooltip` in `@open-planner/ui` that wraps any element and shows a label on
hover or focus, replacing the `title` attributes the browser renders slowly,
inconsistently, and never for keyboard users.

## Shape

```tsx
<Tooltip content="In progress">
  <StatusIcon status="in_progress" />
</Tooltip>
```

The bubble is fixed to the viewport and measured against the wrapped element, so
a row inside the list's scrolling panel does not clip it, and it follows what it
points at while that panel scrolls.

## Users

- The status icon in a list row and on the detail page: the status label.
- `TimeAgo`: the exact instant, which the relative text leaves out.

## Depends on

Row tooltips are dead under the title's stretched link ([[./00047-web-ui-row-tooltips-are-unreacha.md]]):
the overlay takes every hover in the row. This takes that task's third option —
the row navigates from its own click handler, so no overlay covers its cells.

## Verify

Hover a status icon and an age in the list and on a task page: both show a
bubble, and clicking anywhere in the row still opens the task.
