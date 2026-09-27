---
status: in_review
created: 2026-09-27T14:43:18Z
tags:
- feature
- ui
---
# Make diamond nodes less wide and round their corners

A diamond node (`{ }` in a flowchart) is too wide for its height, and its corners are sharp. Make the diamond less wide and give it rounded corners.

## Now

`crates/op-diagram-render/src/graph/size.rs` makes the diagonals twice the padded text box. The label stays on one line when it can, so the text box is wide and low. In the OPP-115 diagram, `openplan.store set?` gets a diamond of 293 by 60 px, near 5 to 1. `crates/op-diagram-render/src/svg.rs` draws the outline as a `polygon` with four sharp points.

## Change

- Wrap the label of a diamond to a narrower width, so that the text box is closer to a square. The diamond must still hold the full label. A diamond must not be more than approximately 2 to 1.
- Draw the outline as a `path` that rounds each of the four corners.
- Make sure that the edges still touch the outline at the rounded top and bottom points, with no gap.
- Update the render snapshots and look at `flowchart/shapes.mmd`, `flowchart/reshape.mmd`, and `tasks/opp-115.mmd`.
