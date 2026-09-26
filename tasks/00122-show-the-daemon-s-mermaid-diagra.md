---
status: todo
created: 2026-09-26T02:01:56Z
tags:
- cli
- docs
- feature
- ui
---
# Show the daemon's Mermaid diagrams and flow in the web app, lint them, and remove D2

The daemon draws Mermaid diagrams and the flow as SVG, but no client uses
it yet. This task connects the web app, `openplan lint`, and the agents to
it, and removes D2, ELK, and React Flow. The design, the Mermaid subset,
and the rules for the SVG are in
[[./00117-draw-mermaid-diagrams-and-the-fl.md]].

## Done before this task

- `op-diagram` (the IR), `op-diagram-mermaid` (the parser), and
  `op-diagram-render` (layout and SVG), with snapshot tests that are SVG
  files.
- `POST /api/diagram` gives `{ svg, width, height }`. A source that does
  not parse gets a 422 with `position` (line and column).
- `GET /api/flow/drawing` takes the query of `/api/flow`. With `width`
  and `height`, each part of the flow gets its own layout, and the parts
  fill the shape of the page. A cycle gets the 422 with `cycles`.
- An empty diagram has a width and a height of zero.
- The daemon keeps the drawings in a cache, up to 16 MB of SVG.
- The generated web client has `drawDiagram` and `drawFlow`.

## Work

1. **Lint.** `openplan lint` parses each `mermaid` fence in task bodies
   and in comments with `op-diagram-mermaid`, and reports the task, the
   line in the task, and the parse error. Lint never starts a daemon, so
   it parses in the CLI.
2. **Viewport.** One component shows an SVG and pans and zooms it. It
   sets a CSS transform on the element that holds the `<svg>`, through a
   ref, and never sets React state for each frame. It adds
   `will-change: transform` during a gesture and removes it about 150 ms
   after the gesture stops. It fits the drawing to the page when it
   opens.
3. **Theme.** One CSS file gives each class of the SVG its colors in the
   light and the dark theme. `crates/op-diagram-render/tests/review.css`
   lists every class that the writer uses. A status icon takes its color
   through `currentColor`.
4. **Font.** Wait for `document.fonts.load()` of Inter Diagram 400 and
   600 before the first paint of a diagram, so that the browser never
   draws one in a fallback font. The widths of the layout are the widths
   of this font only. (This is the rest of
   [[./00116-bundle-one-font-for-diagram-text.md]].)
5. **Task pages.** `task-ui` shows a `mermaid` fence inline with the SVG
   from `POST /api/diagram`, and so does the full view. A parse error
   shows its message and its line in place of the diagram.
6. **Flow page.** Get the SVG from `GET /api/flow/drawing` with the width
   and the height of the page. Ask again after a resize settles. Show the
   empty state when the width is zero, and the cycles as now.
7. **Links.** One click handler on the viewport gives a link that starts
   with `/` to React Router, so a card opens its task without a reload.
8. **Remove** `@terrastruct/d2`, `elkjs`, and `@xyflow/react`, and the
   code that uses them (`flow-layout.ts`, `flow-nodes.tsx`,
   `diagram-block.tsx`, `diagram.ts`). Remove `GET /api/flow` if no client
   reads it any more, and run `mise run generate-web-client`.
9. **Migration.** Convert the 7 D2 blocks to Mermaid: OPP-115, and
   CQR-71, CQR-73, CQR-74, CQR-77, CQR-89, and CQR-90. The conversions
   are in `crates/op-diagram-mermaid/tests/diagrams/tasks/`. Change the CQR
   tasks only with the consent of the user.
10. **Skill and prompt.** Replace the D2 section with the Mermaid subset
    in the three copies of `SKILL.md` (`.agents`, `.claude`,
    `crates/op-skills`) and in the prompt in
    `crates/op-server/src/agent.rs`.

## Acceptance

- The SPA bundle holds no D2, ELK, or React Flow code.
- `openplan lint` reports a broken `mermaid` fence with its task and
  line.
- The 7 converted blocks draw without an error.
- The task page and the flow page show the SVG in the light and the dark
  theme. A click on a card opens its task.
- `cargo build`, `cargo test`, `cargo fmt --check`, `cargo clippy -- -D
  warnings`, and the web tests pass.

## Known limits of the engine

- The `direction` of a subgraph is parsed and then ignored.
- A character outside the font (Cyrillic, CJK) takes a width of 1 em, so
  its label is wider than needed.

## Open question

- Wheel: zoom (as the flow page does now), or pan with pinch and Ctrl
  with the wheel to zoom? Decide it before the viewport work starts.
