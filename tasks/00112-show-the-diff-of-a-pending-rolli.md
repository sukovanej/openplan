---
status: todo
created: 2026-09-09T20:45:26Z
dependencies:
- ./00110-ui-rolling-updates-control-and.md
tags:
- daemon
- feature
- ui
---
# Show the diff of a pending rolling update with a generic DiffView

The review popover from [[./00110-ui-rolling-updates-control-and.md]] lists
each pending task with a change kind and nothing more. A person cannot see what
changed before they publish it. This task shows the diff of each pending task
inside the popover, and adds a generic component that renders a unified diff.
The component knows nothing about tasks or branches. Any later caller with a
unified diff text can reuse it.

## Route

`GET /api/projects/{project}/rolling-updates/{task}/diff` returns the diff of
one pending task as text: `{ diff: string }`.

The server runs `git diff <default-branch> -- <path>` in the rolling-updates
worktree. That command compares the default branch's tree to the working file,
so a dirty edit shows without a read through the index. Git produces the file
header, the `@@` ranges, and the context. The server adds nothing to it.

The route answers 404 when the task has no pending change, and 503 when the
repository has no rolling-updates branch. It takes no branch parameters. It is
specific to rolling updates on purpose. Resolve the task file path through the
store, the same way a write does. The `MatrixCell` carries no path.

Do not add a diff crate. Git already formats the diff.

## `DiffView` component

Add `DiffView` to `@openplan/ui` in `diff-view.tsx`. It takes `diff: string`
and a `className`. It parses the unified format itself: the `---` and `+++`
file lines, each `@@` range line, and the ` `, `+`, and `-` line prefixes. Keep
the parser in `diff-parse.ts` next to it, as pure functions with no React, so
tests read plain input and plain output.

Task files are markdown prose with one paragraph on one line, so a line-only
diff hides what changed inside a paragraph. Add word-level emphasis in the
client from the start. In each hunk, pair a run of removed lines with the run
of added lines that follows it. Diff the pair by words and mark the words that
differ. Use the `diff` npm package's `diffWords`, or a short LCS on words if
the package pulls in more than it earns.

Rendering:

- A grid with two line-number gutters and one text column.
- Menlo, the app's code font. Lines wrap. Prose lines are long.
- A muted `@@` row between hunks that shows the two ranges.
- Added and removed lines use `text-change-added` and `text-change-deleted` with
  a light background tint. An emphasised word gets a stronger tint.
- No syntax highlighting.
- An empty diff renders "No differences".

The file header with the task id and the `ChangeMark` belongs to the caller,
not to the component. The component drops the `---` and `+++` lines from the
output.

## Popover

- Each pending row gets a chevron. A click expands the row and mounts `DiffView`
  under it in a box with `max-h-80 overflow-auto`. One row is open at a time.
- The popover widens from 24rem to 40rem, capped to the viewport width.
- Add `useTaskDiff(project, task)` in the app. It fetches with react-query and
  invalidates on `rolling_updates_changed` and on the task's own change event.
- Add the route to `lib/api.ts` and regenerate the client with
  `mise run generate-web-client`.

## Verify

Server tests in `crates/op-server/tests/rolling_updates.rs`:

- A modified task returns a diff with one `@@` hunk and both a `-` and a `+`
  line.
- An added task returns a diff against `/dev/null` with `+` lines only.
- A deleted task returns a diff with `-` lines only.
- A dirty edit that the daemon has not committed appears in the diff.
- A task with no pending change answers 404.

Web tests:

- The parser in `web/packages/ui/tests`: file lines, ranges, the three line
  kinds, a hunk with no trailing newline, and word emphasis on a paired
  removed and added line.
- `DiffView` in `web/packages/ui/tests`: gutters, line kinds, the hunk
  separator, and the empty state.
- The popover in `web/packages/app/tests/rolling-updates.test.ts`: the chevron
  expands one row, a second click on another row closes the first, and the diff
  query invalidates on `rolling_updates_changed`.
