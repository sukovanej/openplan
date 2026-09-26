---
status: backlog
created: 2026-09-26T13:28:35Z
tags:
- feature
- ui
---
# Show GitHub issue and pull request links as a GitHub icon and #ID

A task body or comment often holds a full GitHub address, for example `https://github.com/milansuk/open-plan/pull/199`. The web UI shows the full address. Show it as a GitHub icon and `#199` instead.

## Requirements

- Change a link to a GitHub issue or pull request (`https://github.com/<owner>/<repo>/issues/<n>` or `.../pull/<n>`) when its text is the address itself. This includes a bare address that `remark-gfm` makes into a link.
- Show the GitHub icon, then `#<n>`. The link keeps its address and opens GitHub.
- Show the full address in the tooltip of the link.
- Keep a link with its own text, for example `[the fix](https://github.com/...)`, as it is.
- Keep an address in inline code or in a code block as literal text.
- Apply the change everywhere the web UI renders task markdown: the task body and the comments.

## Where

`web/packages/task-ui/src/task-body.tsx` renders the markdown with `remark-gfm` and the task-link plugin in `task-links.ts`. Add the change in the same place, as a remark plugin or as the `a` component.
