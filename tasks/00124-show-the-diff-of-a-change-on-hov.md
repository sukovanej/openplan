---
status: backlog
created: 2026-09-26T02:02:39Z
tags:
- daemon
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
- The "and N more" line gets no popover.

## Technical design

A diff is a function of a revision and a path. A revision never changes, so each diff is computed once and then comes from a cache.

```d2
shape: sequence_diagram
line: change line
query: React Query
daemon: daemon
backend: Backend

line -> line: hover delay ends
line -> query: diff(project, revision, path)
query -> daemon: GET only on a cache miss
daemon -> backend: read_at(parent, path)
daemon -> backend: read_at(revision, path)
daemon -> daemon: diff in process, cap the size
daemon -> query: "{ diff, truncated }"
query -> line: DiffView in the popover
```

### Daemon

- Add `GET /api/projects/{project}/revisions/{revision}/diff?path=<path>`. It returns `{ diff: string, truncated: bool }` as a unified diff.
- Take the path from `DocumentChange.path`. This lets one route serve task, tag, config, and asset lines. An optional `from=<path>` gives the parent side a different path, for a renamed task file.
- Read the two sides with `Backend::read_at` at the first parent and at the revision. Both backends already have this method. A missing side is an empty file. Do not check out a tree. Do not start a `git` process.
- Compute the diff in the daemon with one in-process diff crate. `gix` already carries `imara-diff`, so prefer that crate to a new dependency.
- Run the work in `blocking`, as the history routes do.
- Cap the answer at a fixed number of lines (for example 400). Set `truncated` when the cap cuts the diff. Do not diff a binary asset. Return a short marker for it.
- Send `Cache-Control: private, max-age=31536000, immutable`. The answer for one revision and one path never changes.
- Answer 404 for an unknown revision or a path that the revision does not change.

### Web

- Start the query when the hover delay ends (use the `HOVER_DELAY` of `Tooltip`), not on `pointerenter`. A pointer that moves across the list must send no request.
- Put the query key beside `revisionKey`, outside the task keys: `[...projectKey(project), "revision", revision, "diff", from, path]`. Keep `staleTime: Infinity`. The change events must not invalidate this key.
- Read through `abortable`. When the popover closes before the answer arrives, the request stops.
- Keep one open popover for the whole list. Keep the hover state in the change line, so that a hover does not render `RevisionList` or the memoized `Revision` again.
- Restore `DiffView` from `d275e32` (OPP-112, removed in #161) into `@openplan/ui`. Load it and the Shiki `diff` grammar with `React.lazy`, so the activity route chunk does not grow.
- Give the popover a fixed maximum height and width, and let it scroll. Show a skeleton while the diff loads. When `truncated` is set, show a line that says so.
- A task line can hold two paths when a new title renamed the file. Send the removed path as `from` and the added path as `path`.
