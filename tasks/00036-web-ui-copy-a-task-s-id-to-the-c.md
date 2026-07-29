---
status: todo
created: 2026-07-26T14:32:08Z
---
# Web UI: copy a task's id to the clipboard with Cmd+.

`Cmd+.` copies a task's id (e.g. `28`)
to the system clipboard. It works on the list route for the task under the mouse
pointer, and on the detail route for the task being viewed.

## Current state (scouted)

- Bindings are declarative rows in `web/packages/app/src/lib/keys/bindings.ts`,
  each `{id, keys, scope, label, group, run(ctx)}`. `Dispatcher` matches chords,
  `helpGroups` derives the `?` overlay from the same table, so a new binding
  shows up in help for free.
- `normalizeToken`/`fromEvent` (`lib/keys/match.ts`) already canonicalise
  `mod+.` — Cmd and Ctrl both map to `mod`, so `"mod+."` is the authored key and
  Ctrl+. works on Linux/Windows unchanged.
- Modified chords skip the editable-target guard (`hasCommandModifier` short-
  circuits it in `Dispatcher.handleKeyDown`), so `Cmd+.` fires even while a
  combobox input has focus. That is the wanted behaviour here.
- **There is no hover state today.** `TaskRow` styling is pure CSS
  (`hover:bg-muted/30`), and the grid's `onMouseMove` *clears* the keyboard
  cursor (mouse and keyboard selection are deliberately exclusive). Nothing in JS
  knows which row the pointer is over — that has to be added.
- The detail route knows its task only through `useParams()`; `RunContext` has no
  notion of a "current task".
- No toast / transient-feedback component exists anywhere in the app.

## Design

1. `lib/copy-target.ts` — hover tracking + resolution.
   - A tiny external store (same shape as `rowCursor`) holding
     `hoveredId: string | undefined`, with `enter(id)` / `leave(id)`.
     `leave` only clears when the id still matches, so row→row moves don't race.
   - `copyTargetId(hovered, cursorFocused, routeTaskId)` — pure, testable
     resolution in this priority order:
     1. hovered row id (list rows, and detail subtask rows),
     2. keyboard-cursor focused id (`rowCursor` on list, `subtaskCursor` on detail),
     3. the detail route's own task id,
     4. `undefined` → the binding is a no-op.
   - Hover wins over the cursor, which matches the existing rule that moving the
     mouse clears the keyboard cursor.

2. Wire hover into the two row lists.
   - `routes/list.tsx` `TaskRow`: `onMouseEnter` / `onMouseLeave` → `copyTarget`.
   - `routes/detail.tsx` `SubtasksSection` `<li>`: the same.
   - Clear on the scroll container's `onMouseLeave` too, so a hover doesn't
     outlive the pointer leaving the list.

3. `lib/keys` — the binding and its context.
   - `RunContext` gains `copy: { taskId: () => void }`.
   - `useKeyboard` supplies it: reads the hovered id, the active cursor's focused
     id, and the route's task id (parsed from `location.pathname`, no new store),
     feeds them to `copyTargetId`, and hands the winner to the clipboard writer.
   - New binding: `{id: "task.copy-id", keys: "mod+.", scope: "global",
     label: "Copy task id", group: "Task"}`. `global` scope so both routes are
     covered by one row; the resolver decides *which* id. Overlay scope stays
     exclusive, so `Cmd+.` is inert while help is open — correct.

4. `lib/clipboard.ts` + transient confirmation.
   - `writeClipboard(text)` wraps `navigator.clipboard.writeText`. The app is
     only ever served from `localhost`/`127.0.0.1`, a secure context, so no
     `document.execCommand` fallback is needed; a rejected promise is surfaced
     instead of swallowed.
   - A `flash` store (message + tone, auto-clearing after ~1.6s) and a
     `<CopyFlash />` pill rendered once in the app shell, bottom-right, alongside
     the existing `ConnectionStatus`/`MutationError` chrome: `Copied
     28` on success, `Copy failed` on rejection. Same store
     serves any future copy action.

## Acceptance criteria

- Hovering a row in the task list and pressing `Cmd+.` puts that row's id on the
  clipboard, with no click or prior selection.
- With no hover but a `j`/`k` selection, `Cmd+.` copies the selected row's id.
- On `/task/<id>` with nothing hovered or selected, `Cmd+.` copies `<id>`;
  hovering a subtask row copies the subtask's id instead.
- `Ctrl+.` behaves identically (the `mod` alias).
- A short confirmation showing the copied id appears, and a clipboard rejection
  reports failure rather than silently doing nothing.
- The `?` overlay lists the shortcut under **Task** without any extra wiring.

## Testing (`web/packages/app/tests/`, never in `src`)

- `copyTargetId` priority table: hover beats cursor beats route id; all-empty →
  `undefined`.
- Hover store: `leave` on a stale id does not clear a newer hover.
- `normalizeToken("mod+.")` and a `Cmd+.` `KeyboardEvent` produce the same token.
- Dispatcher: `Cmd+.` runs the binding on both route scopes, is inert while the
  overlay is open, and still fires when an input has focus.
- Clipboard: success and rejection each drive the expected flash state
  (`navigator.clipboard` stubbed).

## Out of scope

- Copying anything other than the id (title, URL, markdown link).
- A right-click / context-menu or a click-to-copy affordance.
- A general-purpose toast system beyond the single flash pill.
