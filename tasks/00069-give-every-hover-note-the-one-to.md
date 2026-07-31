---
status: todo
created: 2026-07-31T12:19:40Z
parent: ./00059-web-ui-extract-reusable-componen.md
---
# Give every hover note the one Tooltip

Two hover idioms coexist since [[./00064-web-ui-tooltip-primitive-tooltip.md]] landed: status icons and times use the `Tooltip` primitive, while `BranchTag` (via `Tag`), `TaskRefChip`, `MetaItem` (so `ParentLink` and the problem items in `TaskTimes`) and `IconToggleGroup` still use native `title` — different delay, different look, invisible to keyboard focus.

Migrate those to `Tooltip`, then drop the `title` props from `Tag` and `MetaItem` so the old idiom cannot creep back in.
