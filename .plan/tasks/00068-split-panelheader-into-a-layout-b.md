---
status: in_review
created: 2026-07-31T12:19:40Z
parent: ./00059-web-ui-extract-reusable-componen.md
---
# Split PanelHeader into a layout bar and a PanelTitle

`PanelHeader` imposes `uppercase tracking-wide font-medium` on everything inside it. That suits the list's "Tasks" label, but the detail header puts `TaskIdentity` and the parent link in there, and `HeaderParent` has to undo it twice (`font-normal tracking-normal normal-case` at `routes/detail.tsx:205`, `normal-case` again around the parent picker at `:196`). Content that must un-style its container is the sign the primitive is scoped wrong.

Split it: `PanelHeader` becomes the layout bar (height, border, background, flex) and a new `PanelTitle` carries the uppercase label styling. The list reads `<PanelHeader><PanelTitle>Tasks</PanelTitle></PanelHeader>`; the detail header drops its overrides.
