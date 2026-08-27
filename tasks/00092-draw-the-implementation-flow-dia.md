---
status: todo
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
