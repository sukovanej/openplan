---
status: in_review
created: 2026-08-27T11:13:36Z
parent: ./00090-compute-and-show-the-implementat.md
dependencies:
- ./00091-add-get-api-flow.md
---
# Draw the implementation flow diagram

A diagram at `/flow` draws the implementation flow. The parent task holds the graph rules and the
order. This task draws them.

The endpoint query parameters live in the URL, so a person can bookmark a flow and share it.

## Library

Use `@xyflow/react` to render. A node is a React component, so it keeps Tailwind, the theme, the key
bindings and a router link. A nested node with `parentId` and `extent: "parent"` draws the parent
box.

Use `elkjs` for the geometry. It lays out a nested box, and it routes each line around the nodes it
passes. The layer of a node comes from its `wave`, so the picture and the endpoint agree on the
order. The engine loads when a reader opens the flow, so the app carries none of its 1.5 MB until
then.

Rejected alternatives:

- `dagre` cannot draw a nested box, so it cannot draw a parent.
- `cytoscape.js` draws on a canvas, so a node stops being a React component and stops matching the app.
- `mermaid` gives no interaction.
- A grid of the wave and the position, with no engine. A box holds children whose positions are far
  apart, so the box swallows the tasks between them, and a dependency that skips a wave draws a line
  across the tasks in between.

## Layout

The waves run from top to bottom. Each wave is a row. The arrows then read as "the time moves down".
A card is wide, so a row holds more of them across a page than a column holds down one.

A box takes a block of rows of its own, so a task beside a box draws below the whole box and not
level with the child that shares its wave. Each arrow still runs downwards, and a task is still
drawn after every task it waits for.

Two tasks with no path between them make separate islands. Each island is laid out on its own, and
the islands are packed into rows that fill the shape of the page. A pile of unblocked tasks then
reads across the page instead of running off the bottom.

No node touches another node, and no line crosses a node it does not join.

The graph never draws below a minimum zoom. A card stops being readable first, so a reader pans to
what is left.

## A node

A node shows the task key, the title, a status mark, and the status colour on its whole frame. The
colours match the board.

Show no wave number and no project name. At 50 nodes the graph reads as a shape, and each extra
glyph costs that.

A click opens `/task/:id`.

An unresolved node looks different and shows its raw text.

## Entry points

- A link in the header.
- The command palette.
- A key binding, through the registry in `web/packages/app/src/lib/keys`. Add no special case.
- An action on the task detail page. The action opens `/flow?project=X&task=OPP-42`. This entry point
  makes the feature useful every day: it turns "what blocks this" from a list of links into a picture.

## Client

Run `mise run generate-web-client` after the endpoint lands. It regenerates the Effect client from
the OpenAPI spec.

## Comments

### 2026-09-01T10:25:20Z by Milan Suk via claude-code

> Decisions the task left open:
>
> - The task says the wave is the column and the position is the row. I use the wave as the
>   column, but I compute the row myself. The endpoint sorts each wave on its own, so a box's
>   children can take positions that are far apart, and a box drawn around them would then
>   swallow the tasks between them. Each container packs its entries into rows instead: an entry
>   takes the first row that is free in every column it spans. Two entries that share no column
>   share a row, so a chain of dependencies stays on one line.
> - A box grows outwards from its children. The children keep the column of their own wave, so a
>   box never pushes a child off the grid, and boxes nest to any depth.
> - An unresolved node has no wave, so it takes a gutter column left of wave 0. Each column
>   carries a header: `Wave 0` for a wave, `Unresolved` for the gutter. A column header would
>   otherwise give an unresolved node a wave number the endpoint refuses to give it.
> - The nodes are selectable. React Flow gives a node pointer events only while it is selectable,
>   and without them the pane takes the click that must open the task.
> - `ApiErrorBody` now passes through whole. The client rewrote every refusal body to `{message}`
>   alone, which dropped the `cycles` field of the 422.
> - The status filter goes to the daemon as it stands in the URL. The daemon is what knows the
>   status names, and it refuses one it cannot read with a 400 that names all six.
> - The `home` seat of the command palette is now the general command interface: it lists the
>   commands the app answers for, then the tasks the query finds. `Show the implementation flow`
>   is the first command. `/` still opens the task search alone.
>
> Not done here: the page adds no filter panel. The task names four entry points and puts the
> query in the URL, so a reader narrows a flow through an entry point or through the address bar.
> A `Show every task` link goes back to the whole flow.
>
> Verified against a scratch daemon on this repository's store and on a throwaway store: the
> waves read left to right, `OPP-90` draws as a box around `OPP-92`, a click on a node opens
> `/open-plan/task/OPP-92`, `f` on the detail page and `g f` anywhere go to the flow, an
> unresolved dependency draws in the gutter, a cycle shows `DEM-1 → DEM-2 → DEM-3 → DEM-1`, an
> unknown status shows the daemon's 400, and an empty flow says so. Light and dark both read.
> 274 web tests pass, 658 Rust tests pass, oxlint, oxfmt, clippy and fmt are clean.

### 2026-09-01T10:51:09Z by Milan Suk via claude-code

> The grid is gone. `elkjs` computes the geometry now, and I updated the Library and the Layout
> sections of this task to record it.
>
> Why: the hand-written grid could not hold the three rules the user asked for. Measured on this
> repository's store, the whole graph fitted only at 0.41 zoom, because every task that waits for
> nothing took a row of its own and the graph ran off the bottom; and one line in twelve
> (`OPP-40 → OPP-46`) crossed a task it does not join, because a dependency that skips a wave draws
> straight through the column between. The task said to add the engine "later, and only if the edges
> become hard to read". The user read that condition as met and chose the engine.
>
> What the engine does and does not decide:
>
> - The layer of a node comes from its `wave`, through ELK's `INTERACTIVE` layering. ELK computes no
>   layering of its own, so the picture and the `wave` field cannot disagree. An unresolved node takes
>   the layer left of the first, which no task holds.
> - ELK packs no islands: `elk.aspectRatio` changed nothing in any option I tried. So each island is
>   laid out on its own and this code shelf-packs the islands to the shape of the page. The islands
>   keep the endpoint's order, so the task that unblocks the most work still reads first.
> - The lines are drawn from ELK's own route, through a custom React Flow edge. React Flow would
>   otherwise draw its own line between the two handles, and that line is the one that crossed a node.
>
> After: the same store fits at 0.85 zoom with no line over any node. The floor is 0.5, so a store too
> big for one page stops shrinking there and a reader pans.
>
> Cost: 1.43 MB, in a chunk of its own. The app bundle stays at 893 kB and the engine loads when a
> reader opens `/flow`.
>
> Two tests hold the guarantees: one samples every line and fails on any node it crosses, one compares
> every pair of frames.
>
> Verified: 278 web tests, oxlint and oxfmt clean, `openplan lint` clean, and the page checked in a
> browser in both themes — a click on a card opens its task, and a 700x500 window clamps at 0.5.

### 2026-09-01T11:25:51Z by Milan Suk via claude-code

> A code review found a claim in this task that the drawing does not keep, and I corrected the task
> rather than the drawing.
>
> The claim: "the wave is the column", globally. ELK reads each layer from the wave, so this holds
> between two free tasks. It does not hold across a box. ELK gives a box a block of layers of its own
> and draws every task outside it after the whole block. I reproduced it: a box holding waves 0 to 3,
> beside a free task of wave 1, puts the free task to the right of the box's wave-3 child. A reader
> could take the free task for later work.
>
> I kept ELK, because the two rules the drawing must not break — nothing overlaps, and the whole graph
> fits one page — need it, and because the weaker rule that survives is the one a reader uses: each
> arrow runs left to right, and a task is drawn after every task it waits for. The wave number is no
> longer on the page, so no column claims a number it cannot keep. The Layout section now says this.
>
> The alternative, if the column rule matters more than the two rules above: draw a box as a frame
> around children that stay on a global grid, instead of as a node the engine lays out. That is the
> grid this task started with, and it is what forced the engine in the first place.
>
> Fixed from the same review:
>
> - `Refit` fitted the viewport to the packing it was replacing. The engine runs after the shape
>   changes, so the refit now waits for the drawing that answers the new shape.
> - A resize started a fresh layout on every 0.1 of aspect ratio. ELK runs on this thread, so a drag
>   of the window edge queued blocking layouts. The shape settles for 200 ms first.
> - A failed fetch of the engine was cached for the life of the tab, so `/flow` stayed a skeleton for
>   ever. A failure is forgotten now, and the page says it could not draw.
> - `cardHeight` counted the lines of only the widest over-wide word, and it left the card's border
>   out of both the text width and the height. A title of two very long words overflowed its card.
> - A search the daemon refused took the palette's commands down with it.
> - The arrowhead painted in React Flow's own grey while its line took the theme's.
> - The help overlay listed "Esc — Back" twice, once for the detail page and once for the flow.
>
> Refused: `/flow` takes a name out of the `/:project` namespace, so a project named `flow` is
> unreachable. OPP-91 fixes the path, and the parent task puts the flow above the projects on purpose.
