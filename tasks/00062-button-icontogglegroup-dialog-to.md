---
status: todo
created: 2026-07-30T18:53:07Z
parent: ./00059-web-ui-extract-reusable-componen.md
dependencies:
- ./00060-scaffold-open-planner-ui-and-tas.md
---
# Button, IconToggleGroup, Dialog, Toast and Combobox primitives

Four buttons carry four hand-written class strings, and two notices carry the
same red surface. Give the interactive parts one implementation each.

## `Button`

A `cva` component in the shape of the existing `components/ui/badge.tsx`.
Variants cover what the app already draws: `ghost` (the parent pencil,
`routes/detail.tsx:210`), `accent` (Add subtask, `routes/detail.tsx:300`), and
`danger` (the notice dismiss, `components/mutation-error.tsx:22-28`); sizes
cover `sm` and `icon` (the help-overlay close, `components/help-overlay.tsx:88`).
Focus-visible ring included, so no call site respells it.

## `IconToggleGroup`

The `radiogroup` shell from `components/theme-toggle.tsx` — inset track, one
button per option, active option raised — over a generic
`{ value, label, Icon }` list. `ThemeToggle` stays in the app as the adapter
that binds `useTheme` to it.

## `Dialog`

From `components/help-overlay.tsx:37-100`: backdrop, click-outside to close,
focus move and restore, the `Tab` trap, `aria-modal`, and the title row with its
close button. The shortcut list stays in the app's `HelpOverlay`, which becomes
content inside a `Dialog`.

## `Toast`

`components/flash.tsx` and `components/mutation-error.tsx` draw the same red
surface at two corners. One component: `tone` of `ok` or `danger`, an optional
dismiss action, and a `role` the caller picks — `status` with `aria-live` for
the flash, `alert` for the mutation error. The flash's live region must keep
outliving its messages; a live region inserted together with its text is
announced unreliably. Both app components stay as adapters over their stores.
The `danger` colours come from the tokens, not `red-50` / `red-950`.

## `Combobox`

Move `components/search-combobox.tsx` and `src/lib/fuzzy.ts` into the package
with `FuzzyText`. It is already generic over `buildOptions`; the debounce,
keyboard handling, outside-click dismissal and inline/overlay placement come
along unchanged, and its options now render through `Row`.

This is the overlay primitive the ⌘K palette asks for
([[./00031-reusable-k-palette-component-tas.md]]): same contract — the component
owns the chrome and keyboard, the consumer supplies the items. That task builds
its palette on this `Combobox` rather than a second one.

## Tests

`tests/fuzzy.test.ts` moves to the package. Add tests for `Dialog` (focus
restored on close, `Tab` wrapping) and `Toast` (both tones, dismiss fires).

## Verify

Checks, then interactively: the parent picker and the add-subtask box open,
filter, choose with `↑` `↓` `↵`, and dismiss on `Esc` and outside click; `?`
opens the shortcut overlay and focus returns where it was; a failed mutation
shows the error notice and dismisses; a copy flashes.
