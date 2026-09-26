---
status: backlog
created: 2026-09-26T02:02:39Z
tags:
- feature
- ui
---
# Show the diff of a change on hover in the activity view

When the user hovers over a change line in the activity view, show the diff of that change in a popover.

The activity view (`web/packages/app/src/routes/activity.tsx`) lists revisions with `RevisionList`. Each revision has change lines for tasks, tags, and documents. Now a line tells only the kind of change (added, modified, deleted) and what it changed.

- Show the diff between the revision and its parent, for the file of that one change line.
- Load the diff only when the popover opens.
- Keep a click on the line as it is now.
- Show the popover on keyboard focus too.
