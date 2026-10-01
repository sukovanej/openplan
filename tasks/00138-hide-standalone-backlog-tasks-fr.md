---
status: done
created: 2026-09-27T18:23:01Z
tags:
- daemon
- feature
- uiasldkfjasdf
pull_requests:
- https://github.com/sukovanej/openplan/pull/237
---
# Hide standalone backlog tasks from the flow and put the key and title of a card on one line

The flow page (`/flow?project=open-plan`) shows too many cards, and each card uses more lines than it must.

## Backlog tasks do not seed the default flow

A backlog task is not ready for work. The default flow (no status and no key in the query) must not show a backlog task that no other shown task needs.

- In `FlowQuery::selects` (`crates/op-api/src/flow.rs`), `is_remaining` makes every task that is not `done` or `cancelled` a seed. Remove `backlog` from the seeds of the default flow.
- `grow` still pulls in a backlog task when a shown task needs it: a dependency of a shown task, a child of a shown parent, or the parent of a shown task. Keep that.
- A query that names `status=backlog`, or names a task key, still shows backlog tasks. Keep that.

Today the default flow of open-plan shows 8 backlog tasks that are not connected to other work.

## The status, the key, and the title on one line

Today a card puts the key on the first line and the title on the lines below it (`node` in `crates/op-diagram-render/src/graph/size.rs`). A box header does the same (`Header`).

- When the status icon, the key, and the title fit in the width of the card, draw them on one line.
- When they do not fit, draw the icon and the key on the first line. Wrap the title on the lines below.

```text
fits:        [icon] OPP-42  Ship login page
too long:    [icon] OPP-42
             Section-level markdown editing of task
             bodies (engine → CLI → API → web editor)
```

- Keep one width for all cards, so the cards of a flow stay in columns.
- Keep the card heights integer. A fractional height gives half-pixel borders on a Retina display.
- Do the same for the header of a box.
