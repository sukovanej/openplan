---
status: in_progress
created: 2026-09-27T14:51:41Z
tags:
- feature
- ui
---
# Show the agent as an icon with the name in a tooltip

The task history box shows the agent as a `Bot` icon. The agent name shows in a tooltip when the pointer is on the icon. Refer to `Who` in `web/packages/app/src/components/revision-list.tsx`, in the `compact` case.

Other places show the agent as `AgentTag`: a chip with the icon and the full name. Show the agent in these places the same way as the task history box:

- The author line of the task detail page (`TaskAuthor`).
- The comment thread (`comment-thread.tsx`).
- The revision header (`RevisionMeta`).

## Acceptance

- Each place shows only the `Bot` icon for the agent.
- The tooltip on the icon says `via <agent>`.
- The icon has the same `aria-label`.
- One component draws the icon and the tooltip. `Who` in the task history box uses it too.
