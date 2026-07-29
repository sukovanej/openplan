---
status: done
created: 2026-07-14T10:53:32Z
---
# Web UI: light / dark / system theme with persistence

## Goal

Let a user choose the UI theme — **light**, **dark**, or **system** (follow the
OS) — from the app shell, with the choice persisted across reloads and applied
without a flash of the wrong theme on first paint.

The dark palette already exists: `src/index.css` defines a full `.dark` token
set plus `@custom-variant dark (&:is(.dark *))`, but nothing ever adds the
`.dark` class, so the app is permanently light. This task delivers the missing
switching machinery and the control to drive it — from [[bootstrap-the-realtime-web-ui-re-3d0c]].

## The three modes

- `light` — force the light palette.
- `dark` — force the dark palette.
- `system` — follow `prefers-color-scheme`, updating **live** when the OS flips
  while the app is open.

## Preference vs. resolved theme

Keep these two distinct:

- **Preference** — what the user picked: `light | dark | system`. Default
  `system`. Persisted in `localStorage` under a stable key (e.g. `oplan.theme`);
  an unset or unrecognized value falls back to `system`.
- **Resolved theme** — what actually renders: `light | dark`. For `light`/`dark`
  it equals the preference; for `system` it is derived from
  `window.matchMedia("(prefers-color-scheme: dark)").matches`.

**Apply** the resolved theme by toggling the `.dark` class on
`document.documentElement` (matching the existing `@custom-variant`), and set
`document.documentElement.style.colorScheme` to `light`/`dark` so native
controls (scrollbars, form widgets, date pickers) match.

## No-flash first paint

Add a small **blocking inline script** in `index.html` `<head>`, before the
module bundle, that reads the stored preference (or system fallback) and sets
the `.dark` class + `color-scheme` before first paint. It must be self-contained
and independent of the JS bundle — by the time React mounts, the correct theme
is already on `<html>`.

## React integration (`src/lib/theme.ts`)

A small theme module alongside the existing `lib/` (`store.ts`, `runtime.ts`
patterns): read/write the preference, resolve it, apply it, and subscribe to
changes.

- A hook/context so components read the current preference and set a new one;
  setting persists and re-applies synchronously.
- When the preference is `system`, keep a live `matchMedia` `change` listener and
  re-resolve on OS flips; for `light`/`dark`, no media listener is active.
- **Cross-tab sync**: handle the `window` `storage` event so changing the theme
  in one tab updates the others — consistent with this app's realtime,
  multi-tab framing.

## UI control (header, `App.tsx`)

A control in the header to pick light / dark / system — a three-way segmented
control or a dropdown with sun / moon / monitor affordances — styled
consistently with the shadcn primitives in `src/components/ui/`. If a
`dropdown-menu` / `toggle-group` primitive is needed, add it in the shadcn
style already used here.

## Deferred — do not build

- Per-task or per-route themes.
- Custom accent / brand colors, theme editor, high-contrast mode.
- Elaborate cross-fade animation (a minimal transition is fine).

## Tests (vitest)

- Preference round-trips through `localStorage`; unset / invalid → `system`.
- `resolve()`: `system` follows a mocked `matchMedia`; `light`/`dark` ignore it.
- `apply()` toggles `.dark` and sets `color-scheme` on `documentElement`.
- In `system` mode, firing a `matchMedia` `change` flips the resolved theme;
  in `light`/`dark` mode it does not.
- `storage`-event handler updates the resolved theme across tabs.

## Done when

- The header control switches between light / dark / system, and the choice
  survives a reload.
- `system` tracks the OS live while the app is open.
- No theme flash on initial load.
- Web package checks pass (lint + vitest + build).
