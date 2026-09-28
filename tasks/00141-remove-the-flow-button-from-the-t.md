---
status: todo
created: 2026-09-28T13:01:40Z
tags:
- feature
- ui
---
# Remove the Flow button from the task box header

The task box header shows a Flow link next to the status. The link opens the
flow of the task. Remove the link from the header.

## Scope

- Remove `FlowAction` and its use in `web/packages/app/src/routes/detail.tsx`.
- Remove the `Waypoints` import from that file if nothing else uses it.
- Keep the `f` key (`task.flow`). It still opens the flow of the task.
- Keep `taskFlowPath` and the Flow page. The key and the page navigation use them.

## Check

- The task box header shows no Flow link.
- The `f` key on a task opens the flow of that task.
- The header layout stays correct on desktop and on a phone. `FlowAction` set
  `ml-auto`, so check the position of the items that follow it.
