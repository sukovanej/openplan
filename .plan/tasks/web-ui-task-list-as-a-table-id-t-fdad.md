---
status: done
---
# Web UI: task list as a table (id · title · status) with a row cursor

## Goal

Replace the card list in `routes/list.tsx` with a table: `id · title · status`
(title truncates, status as a badge). This establishes the table and its
focused-row cursor state that the keyboard-shortcut work drives.

## Scope

- **Table**: `id · title · status` columns; title truncates; status as a badge.
- **Keyboard cursor**: a focused-row index in small state, clamped to the row
  count, reset when the task set changes. This task owns the cursor *state*;
  the movement bindings (`j` / `k` / `Enter`) are wired by
  [web-ui-keyboard-shortcut-archite-4219].
- **a11y**: roving `tabindex` (or `aria-activedescendant`), a visible focus ring,
  correct `role` semantics; clicking a row also sets the cursor.

## Tests (vitest)

- Cursor clamps to the row count (no index past the last row or before the first).
- Cursor resets when the task set changes.

## Done when

- `routes/list.tsx` renders a table (`id · title · status`, title truncating,
  status as a badge) in place of the card list.
- A focused-row cursor lives in small state, clamped and reset on task-set change,
  and settable by clicking a row.
- a11y semantics (roving tabindex / `aria-activedescendant`, focus ring, roles)
  are in place.
- Web package checks pass (lint + vitest + build).
