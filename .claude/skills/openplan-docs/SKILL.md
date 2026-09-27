---
name: openplan-docs
description: Docs in this repo are "docs" kept by the `openplan doc` CLI, NOT markdown files in the checkout or documents of another docs tool or connector. Invoke this skill whenever the user mentions a doc or docs, by name or not: to read, list, find, write, create, edit, nest, rename, or delete a doc, or to base work on one.
---

# Docs

The `openplan` binary keeps the project docs beside the tasks. A doc is not a
file in the checkout. Read and write docs only with `openplan doc`. Write a new
doc with `openplan doc create`, never as a markdown file in the repository.

A doc name has lowercase letters, digits, and hyphens. `create` and `rename`
print the name they normalize to. Use that name in the next commands.

```sh
openplan doc list                          # the doc tree: name, title
openplan doc get <name>                    # "# Title", then the body
openplan doc get <name> --json             # metadata, parent, children, body
openplan doc create "Release process" --body "…" --parent <name>
openplan doc set <name> "$(cat body.md)"   # replaces the whole body
openplan doc nest <name> <parent>          # "-" moves it to the top level
openplan doc rename <from> <to>            # also changes the title
openplan doc delete <name> --yes
```

To edit a body, write the output of `doc get` to a file, remove the `# Title`
line, edit the file, and `doc set` it. `doc set` keeps the title, so a body
that starts with the title heading shows it twice.

In a body, write `[[OPP-42]]` to link a task and `[[<name>]]` to link a doc.
The write turns each link into a file path.

In a reply to the user, link each doc to its page in the web UI:
`[<name>](<address>)`. Get the address from `openplan url <name>`.

After you create or change a doc, show its page in the browser pane of the Claude desktop app.
Open the address from `openplan url <name>` with the `navigate` tool of the pane (`mcp__Claude_Browser__navigate`).
When you write more than one doc in a row, show the last one. When the browser
already shows the page, do not open it again, because the page updates itself.
Skip this step after `openplan doc delete`, and skip it when the session cannot
use that browser. Do not open the address in a different browser.

`doc list` marks a doc that has a sync conflict with `[conflict]`. Settle the
conflict before you edit the doc. `doc get` prints the steps on stderr.
