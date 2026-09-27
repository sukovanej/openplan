---
status: todo
created: 2026-09-26T13:28:35Z
tags:
- daemon
- feature
- ui
---
# Show GitHub issue and pull request links in a short form with the GitHub icon

A task body or comment often holds a full GitHub address, for example `https://github.com/milansuk/open-plan/pull/199`. The web UI shows the full address. Show a short form instead. When the link points to the repository of the project, the short form is the GitHub icon and `#199`. When it points to a different repository, the short form also names that repository.

## Examples

The project repository in these examples is `milansuk/open-plan`.

| Address | Short form |
|---|---|
| `https://github.com/milansuk/open-plan/pull/199` | icon `#199` |
| `https://github.com/milansuk/open-plan/issues/42` | icon `#42` |
| `https://github.com/rust-lang/cargo/issues/1234` | icon `rust-lang/cargo#1234` |

When the project has no GitHub repository, every link shows `owner/repo#<n>`.

## The project repository

The web UI does not know the repository of the project now. The daemon reads it from the git remote that the project syncs with (`origin` by default) and sends it in `ProjectView`.

OPP-136 needs the same remote and the same short form. The task that merges first adds the parts below, and the other task uses them.

```mermaid
flowchart LR
  remote[(git remote URL)] -->|op-forge parses| daemon[Daemon]
  daemon -->|ProjectView.forge| web[Web UI]
  web --> body[Task body and comments]
```

- Add a crate `op-forge` that parses the remote URL. Parse these forms: `https://<host>/<path>(.git)`, `git@<host>:<path>(.git)`, and `ssh://git@<host>/<path>(.git)`. `github.com` is GitHub. OPP-136 adds GitLab.
- Add an optional field `forge` (`{ kind, host, repo }`) to `ProjectView` in `crates/op-api/src/project.rs`. Leave the field out when the project has no remote, or when `op-forge` does not know the host.
- Regenerate the web client with `mise run generate-web-client`.
- Put the short form in one component in `web/packages/task-ui`. The "Pull requests" section of OPP-136 uses the same component.

## Requirements

- Change a link to a GitHub issue or pull request (`https://github.com/<owner>/<repo>/issues/<n>` or `.../pull/<n>`) when its text is the address itself. This includes a bare address that `remark-gfm` makes into a link.
- Compare `owner/repo` without case, because GitHub names are not case-sensitive.
- The link keeps its address and opens GitHub.
- Show the full address in the tooltip of the link.
- Keep a link with its own text, for example `[the fix](https://github.com/...)`, as it is.
- Keep an address in inline code or in a code block as literal text.
- Apply the change everywhere the web UI renders task markdown: the task body and the comments.

## Where

`web/packages/task-ui/src/task-body.tsx` renders the markdown with `remark-gfm` and the task-link plugin in `task-links.ts`. The task-link plugin gets the project abbreviation through `TaskLinkSource`. Send the project repository to the new code in the same way.
