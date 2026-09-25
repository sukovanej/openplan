---
status: backlog
created: 2026-09-25T15:18:48Z
tags:
- daemon
- feature
- ui
---
# Bundle one font for diagram text and give the daemon its glyph widths

The daemon will lay out diagrams, and the browser will draw them. Layout
needs the width of each label in the font that the browser draws. The UI
uses the system font, which is different on each operating system, so the
daemon cannot know it. One bundled font removes the difference.

## Scope

1. Choose the font. It must have an open license (OFL or similar), Latin
   and Latin Extended-A (Czech labels exist), and tabular digits for task
   keys. Inter is the first candidate. Ask the user before the work
   starts.
2. Use static weights, not a variable font, because a width table for a
   variable axis is much larger. Take only the weights the diagrams use,
   for example 400 and 600.
3. Subset the font to Latin, Latin Extended-A, general punctuation, and
   the arrows that labels use. Ship it as woff2. The target is 150 KB or
   less for all weights together.
4. Serve the font from the SPA with `@font-face`. Draw all diagram text
   in it, and the flow cards too. The browser must not paint a diagram in
   a fallback font: wait for `document.fonts.load()` before the first
   paint.
5. Generate a glyph width table once and commit it as Rust source in the
   crate `op-diagram-render`. Create the crate if it does not exist yet.
   The table holds the advance width of each code point in font units,
   the units per em, the ascender, and the descender. A `mise` task
   regenerates it. The tool that the generator uses is not a dependency
   of any crate.
6. Add `text_width(text, size, weight)` and the line metrics to
   `op-diagram-render`. A code point that is not in the table takes a
   fixed fallback width (1 em for a wide character).

Kerning is not in the table. The width that Rust gives must be the same
as or a little more than the width in the browser, so a label never
overflows its shape.

## Acceptance

- A headless Chromium test measures a corpus of labels (Czech, digits,
  punctuation, long titles) with `measureText` in the bundled font. For
  each label, the Rust width is not less than the browser width and not
  more than 3% above it.
- The SPA build contains the font, and `mise run install` serves it.
- No runtime dependency is added.

## Open question

- Should the whole UI change to this font, or only the diagrams and the
  flow cards? Recommendation: only the diagrams and the flow cards for
  now.
