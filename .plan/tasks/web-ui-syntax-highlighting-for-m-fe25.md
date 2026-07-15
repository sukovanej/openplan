---
status: todo
---
# Web UI: syntax highlighting for markdown code fences

Fenced code blocks in task bodies render as plain `<pre><code>` on a muted
background (`web/packages/app/src/components/task-body.tsx`) with no token
colouring. Add language-aware syntax highlighting with **Shiki** (locked),
integrated so git-diff highlighting can be added later without swapping engines.

## Current state (scouted)

- `TaskBody` uses `react-markdown@10.1.0` + `remark-gfm` + a custom
  `remarkTaskLinks` tree plugin. No `rehype` plugins today. Fenced blocks get
  the app's prose styling: `prose-pre:bg-muted prose-pre:text-foreground`, and a
  `[&_pre_code]` reset so inline-code chip styles don't leak into blocks.
- Dark mode is a `.dark` class on `<html>` (`document.documentElement`), driven
  by a `useSyncExternalStore` singleton (`lib/theme.ts` `themeStore`, exposing
  `useTheme()` → `resolved: "light" | "dark"`). Tailwind v4:
  `@custom-variant dark (&:is(.dark *))`. Colours are oklch CSS vars in
  `index.css` (`--muted`, `--foreground`, …).
- SPA only — Vite + `@vitejs/plugin-react`, no SSR, so client-only async init
  has no hydration-mismatch concern.
- Dep versions live in the pnpm `catalog` (`pnpm-workspace.yaml`).

## Core constraint: sync render vs async highlighter

`react-markdown@10` runs the unified pipeline **synchronously**, so an **async**
rehype plugin (`@shikijs/rehype`) cannot be used — it would throw in the sync
processor. Therefore highlighting must be **synchronous at render time**, which
dictates the design:

- Create a Shiki highlighter with `createHighlighterCore` + the **JS RegExp
  engine** (`@shikijs/engine-javascript`, `createJavaScriptRegexEngine()`).
  The JS engine drops the oniguruma WASM **and** enables synchronous
  `codeToHast`. Creation itself (dynamic grammar/theme imports) is async and
  happens **once**.
- Preload a fixed, curated language + theme set into that one highlighter.
  Highlighting is then a sync call. We do **not** load languages on demand per
  block — that would reintroduce async into the sync render path and cause
  flashes. The whole Shiki module is instead one lazily-imported chunk (below).

## Design

Three pieces, mirroring the existing `themeStore` pattern for the async-ready seam.

1. `lib/highlighter.ts` — the engine singleton.
   - `ensureHighlighter(): Promise<void>` builds the core highlighter once
     (idempotent), with the JS engine, curated langs, and both themes.
   - `highlightToHast(code, lang): Root | null` — sync; returns `null` before
     ready or for a block that should fall back.
   - `useHighlighterReady(): boolean` via `useSyncExternalStore` (same shape as
     `themeStore`): subscribing kicks `ensureHighlighter()`, flips to `true`
     when built, triggering one re-render from plain → highlighted.
   - `resolveLang(raw): BundledLanguage | null` — map/normalise the fence tag to
     a curated language; unknown/absent → `null` (fallback).
   - Curated languages (task-body realistic set): `ts tsx js jsx json rust bash
     sh toml yaml md html css sql python go diff`. `diff` included now so the
     grammar is present when diff rendering lands.
   - Themes: one light + one dark VS Code theme (e.g. `github-light` /
     `github-dark`) whose palettes read well on `--muted`.

2. `components/code-block.tsx` — the render seam.
   - `react-markdown` `components` overrides: `pre` unwraps to `<>{children}</>`
     (we emit our own `<pre>`), `code` decides inline vs block. Inline code (no
     `language-*` class) passes through untouched to keep the existing chip
     styling. Block code calls `highlightToHast`; on `null` (not-ready / unknown
     lang) it renders today's plain `<pre><code>` so there is never a blank
     flash or layout shift.
   - HAST → React via `hast-util-to-jsx-runtime` (no `dangerouslySetInnerHTML`).
   - Memoise per `(code, lang, resolvedTheme)` so re-renders don't re-highlight.

3. `task-body.tsx` — wire the overrides into the existing `components` map;
   nothing else about the prose styling changes.

## Theming integration

- Highlight with Shiki **dual themes**: `codeToHast(code, { themes: { light,
  dark }, defaultColor: false })`, emitting `--shiki-light` / `--shiki-dark` CSS
  vars per token instead of a baked colour.
- Add one small CSS rule keyed off the existing `.dark` variant so tokens switch
  with the app theme — no JS re-highlight on theme change (the vars already
  carry both). This is why `code-block` reads `resolved` only for memo keying,
  not to pick a theme.
- **Keep the current container look**: strip Shiki's own `pre` background
  (transformer or CSS) and keep `prose-pre:bg-muted`, so blocks look identical
  to today except that tokens are now coloured. Avoids double theming.

## Loading & bundle strategy

- The Shiki module (engine + curated grammars + 2 themes) is a **single dynamic
  chunk**, code-split out of the initial route bundle. Fine-grained
  `createHighlighterCore` imports keep it to the curated set only — no
  `shiki/bundle/full`.
- Trigger `ensureHighlighter()` when the first `TaskBody` mounts (detail route),
  not at app boot, so the list route stays lean.
- Before the chunk resolves, plain fenced blocks are shown; the swap to coloured
  is a single re-render.

## Diff-forward seam (build the seam, not the feature)

- `highlightToHast` takes an optional `transformers` array; diff rendering later
  passes Shiki's `transformerNotationDiff` / a diff decorator without touching
  callers.
- `diff` grammar is already in the curated set.
- Rationale recorded so it isn't relitigated: diff content is *fragments*, and
  diff structure (added/removed/word-diff) is a decoration layer painted on top
  of the same token highlighting — Shiki transformers cover this; the engine
  does not change.

## Fallback behaviour

- Highlighter not yet built → plain block (no flash, no shift).
- Unknown / missing language tag → plain block, no error.
- `highlightToHast` throwing (bad grammar edge case) → caught, plain block.

## Testing (`tests/`, per repo rule — never in `src`)

- `resolveLang`: known aliases map, unknown/empty → `null`. Pure, node env.
- Highlighter singleton: `ensureHighlighter()` idempotent; `highlightToHast`
  returns `null` before ready, HAST after.
- `code-block` rendering: block with a known lang → `.shiki` output containing
  token `<span>`s carrying `--shiki-dark`; inline code → unchanged chip;
  unknown lang → plain `<pre><code>`. DOM-dependent — this file needs the
  `happy-dom` env (shared vitest config is `environment: "node"`; override per
  file or via the app config).

## Out of scope

- CLI / terminal highlighting (no terminal markdown rendering exists yet).
- Actual git-diff rendering (only the seam here).
- Copy-button, line numbers, line highlighting, filename headers.

## Acceptance criteria

- A ` ```ts `/`rust`/`bash`/etc. fence in a task body renders with coloured
  tokens in both light and dark, matching the app theme with no re-highlight on
  toggle.
- Inline code chips are visually unchanged.
- Unknown-lang and untagged fences render as today (plain), no errors.
- Initial list-route bundle does not grow by the Shiki payload (verify it is a
  separate chunk); no oniguruma WASM in the build (JS engine).
- `cargo`-side unaffected; `pnpm --filter app build`, `test`, typecheck,
  and lint all pass.

## Dependencies to add (pnpm catalog)

`shiki` (or `@shikijs/core` + `@shikijs/engine-javascript` +
`@shikijs/langs` + `@shikijs/themes`) and `hast-util-to-jsx-runtime`.
