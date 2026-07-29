---
status: done
created: 2026-07-14T10:31:45Z
dependencies:
- ./00011-web-ui-task-list-as-a-table-id-t.md
---
# Web UI: keyboard-shortcut architecture + navigation shortcuts (Linear-style)

## Goal

Introduce a keyboard-shortcut architecture in the web app and use it to ship
read-only **navigation** shortcuts as the showcase. The architecture is the real
deliverable: extensible enough to later carry a `Cmd/Ctrl+K` palette and
HTTP-backed mutation commands (status, create, assign) without touching the
dispatcher.

Scope now: navigation only. No writes, no palette, no multi-select.

## Architecture

A shortcut **registry** + a single global **dispatcher**, in a dedicated module
(e.g. `src/lib/keys/`).

- **Binding**: `{ id, keys, scope, when?, label, group, run }`
  - `keys`: a single key (`"j"`) or a chord sequence (`["g", "l"]`).
  - `scope`: `global | list | detail` — which route(s) the binding is live in.
  - `when?`: optional predicate for conditional bindings.
  - `label` + `group`: text the help overlay renders from.
  - `run(ctx)`: the effect (navigate, move cursor, toggle overlay).
- **Dispatcher**: one keydown listener attached once at the app root.
  - Resolves the active binding set from the current route's scope + `global`.
  - Buffers chord sequences with a timeout (~1s) that resets on completion,
    mismatch, or expiry.
  - Suppresses single-key bindings when focus is in an editable target (input,
    textarea, `contenteditable`, the rendered markdown body).
  - No mod-key bindings needed yet, but the matcher normalizes `Cmd`/`Ctrl` so
    the future palette is a drop-in.
- **Extensibility contract** (must hold; call it out in the PR):
  - Adding `Cmd/Ctrl+K` = one new binding whose `run` opens a palette that reads
    the registry. No dispatcher change.
  - Adding a mutation (e.g. `s` = set status) = one new binding whose `run`
    performs an HTTP call. No dispatcher change.

## List table (prerequisite)

The task list is rendered as a table with a focused-row cursor by
[11] — a dependency of this task. The `j` /
`k` / `Enter` bindings below drive that cursor; this task does not build the
table or its cursor state itself.

## Shortcut set (all read-only)

| Keys        | Scope           | Action                                     |
| ----------- | --------------- | ------------------------------------------ |
| `j` / `k`   | list            | move row cursor down / up                  |
| `Enter`     | list            | open the focused task                      |
| `Esc`       | detail / overlay| back to the list / close the help overlay  |
| `?`         | global          | toggle the shortcuts help overlay          |
| `g` `l`     | global          | go to the task list (chord-engine showcase)|

## Help overlay

Dialog opened by `?`, generated entirely from the registry (single source of
truth), grouped by `group`. Labeled, focus-trapped, closes on `Esc` / `?`.

## Deferred — do not build

- All writes / mutations (arrive later as HTTP-backed commands).
- `Cmd/Ctrl+K` command palette.
- Search / filter UI (no `/`).
- Multi-select (`x`), bulk actions, arrow-key aliases, `gg` / `G` edges.

## Tests (vitest)

- Chord buffering: partial `g` then timeout resets; full `g l` fires once.
- Input scoping: single-key bindings ignored while focus is in an editable element.
- Cursor clamping: `j` at the last row and `k` at the first row are no-ops.
- Help overlay content is derived from the registry.

## Done when

- The `j` / `k` / `Enter` bindings drive the table row cursor from
  [11].
- Every shortcut above works, chords included, with input-scope suppression.
- `?` help overlay renders from the registry.
- Registry/dispatcher are the documented extension point for palette + mutations.
- Web package checks pass (lint + vitest + build).
