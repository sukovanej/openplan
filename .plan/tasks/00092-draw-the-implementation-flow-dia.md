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

Add no layout engine. The endpoint returns the wave and the position, so a grid gives the
coordinates: the wave is the column and the position is the row. The size of a box comes from its
children.

Rejected alternatives:

- `dagre` cannot draw a nested box, so it cannot draw a parent.
- `cytoscape.js` draws on a canvas, so a node stops being a React component and stops matching the app.
- `mermaid` gives no interaction.
- `elkjs` lays out nested boxes well, but it adds about 1.5 MB to a bundle that the binary embeds.
  Add it later, and only if the edges become hard to read.

## Layout

The waves run from left to right. Each wave is a column. The arrows then read as "the time moves to
the right". A node is wide, so a column stacks the nodes better than a row does.

## A node

A node shows the project, the task key, the title, and a status colour on the left border. The colour
matches the board.

Show the position number on the wave column, not on each node. At 50 nodes the graph reads as a
shape, and each extra glyph costs that.

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
