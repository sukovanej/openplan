---
status: in_review
created: 2026-08-23T23:09:52Z
---
# Comments on tasks (CLI, API, read-only web thread)

## Goal
Give every task an append-only comment log that lives in the task markdown file. An agent or a
person adds an entry from the CLI, and every surface reads it: the CLI, the HTTP API, and the web
UI. The log records who wrote the entry and which tool typed it.

## Decisions (locked)
- **Append-only.** The CLI adds an entry. It never edits and never deletes one. A person can still
  correct the file by hand.
- **Flat.** There are no replies and no comment ids. Chronology plus markdown quoting is the thread.
- **Whole-task.** A comment attaches to the task, never to a section. A section anchor has no
  identity that survives a heading rename, so it would point at the wrong place after one edit.
- **Branch-local writes.** A comment goes into the task file, so it belongs to the branch that wrote
  it and reaches `main` only through a merge. Reads are branch-aware, like `get` and `list`.
- **A merge conflict stays a manual fix.** The merge driver gets no comment rule in this task.

## File format
The `## Comments` section is always the last section of the body. The writer creates it when the
first comment arrives. One comment is a `###` heading, then a blockquote that holds the text.

```md
## Comments

### 2026-08-24T09:12:04Z by Milan Suk via claude-code

> # Any heading works
>
> Lists, fences, and tables work too, because every content line carries `> `.

### 2026-08-24T09:20:41Z by Milan Suk

> More text.
```

- Heading grammar: `### <RFC3339 UTC, whole seconds> by <author>[ via <agent>]`. The agent splits on
  the **last** ` via ` in the line.
- The content is the blockquote below the heading. The writer prefixes each line with `> ` and each
  blank line with `>`. The reader strips the prefix. The transform is mechanical and lossless.
- Nothing inside a comment can forge a delimiter, because the content is quoted. An unquoted `###`
  line is an entry heading, and a heading of any level — `#` included — is content of the quote that
  holds it.
- The file order is the true order. A timestamp is a label, not a sort key. Two identical headings
  are legal.
- A heading nested in a blockquote must not reach the document outline. `op_md::headings` collects
  every heading today, so it must skip a heading inside a quote. That fix is correct for a task body
  in general, not only for comments.
- An entry heading is **not an addressable target**. Section addressing, and the splice engine in
  [[./00007-section-level-markdown-editing-o.md]], skip every heading inside `## Comments`, so no
  `-t` path and no `OPP-42#<section>` reference can reach into the log. The log is append-only.

## Identity
The CLI resolves both parts, because only the CLI process sees the environment and the parent
processes. There is no `--author` flag, no `--agent` flag, and no environment override.

- `author`: git `user.name`. The command fails when it is unset, and the error says to set it. An
  unsigned entry in an append-only log is worse than no entry.
- `agent`: `CLAUDECODE` / `CLAUDE_CODE_ENTRYPOINT` → `claude-code`; `CODEX_SANDBOX*` → `codex`;
  `AI_AGENT` used as it stands; then an ancestor process whose binary name is in
  `{claude → claude-code, codex, opencode, aider, cursor-agent, gemini, amp}`; then none.
- The token carries no version. An unrecognized ancestor gives no agent, so a person at a shell gets
  a plain `by <name>`. The ancestor scan is necessary because a shell is also an ancestor, and only
  a known list separates a tool from a shell.
- Both fields are a claim, not a proof. Every signal is spoofable, and the daemon writes what the
  CLI sends.

## Timestamp
The daemon stamps the entry at write time with `op_task::now()`. The daemon is the single in-band
writer, so one clock orders every write. The CLI sends text, author, and agent only.

## CLI
```sh
openplan comment <key> "text"          # add; --body-file <path|-> for a file or stdin
openplan comments <key>                # read, oldest first, every entry
openplan comments <key> --json
openplan comments <key> --branch <name>
openplan comments <key> --all-branches
```
- Empty or whitespace-only text is refused.
- A write targets the caller's branch. A task that is not on that branch is refused, and the error
  names the branches it lives on, like `write_not_found` does today.
- `--all-branches` sorts branches against each other by timestamp, keeps the file order inside each
  branch, breaks a tie with the branch name, and labels each entry with its branch.

## Parse model
A read never fails. A hand-damaged entry keeps its text and reports the broken field, so the UI
shows all the information it has and still marks the data as invalid.

```rust
pub struct Comment {
    pub at: Field<Rfc3339>,
    pub author: Field<String>,
    pub agent: Option<String>,
    pub text: String,
}
```
- `agent` is `Option`, not `Field`: an absent agent means a person typed it, which is normal.
- `text` never fails. Whatever the quote holds is the comment.
- `FieldError::Invalid { message }` carries the offending text, for example
  `not an RFC3339 UTC timestamp: "yesterday"`.
- A quote with no entry heading above it becomes an entry with `at` and `author` as
  `Field::Error(Missing)` and the text intact. A person who writes an unquoted blank line inside a
  comment splits it in two, and this is what the reader then shows.

## API
- `GET /api/projects/{project}/tasks/{id}/comments?branch=` returns a flat list for one branch.
- The all-branches read returns groups of `{ branch, comments }`, so no entry repeats a constant.
- `POST /api/projects/{project}/tasks/{id}/comments` takes `{ text, author, agent }`.
- `TaskDetail` gains `comments`, and its `body` **excludes** the `## Comments` section. The daemon
  strips it, so every client shares one parser and no client renders the thread twice.
- `TaskListItem` gains the comment count beside `metadata`. `Metadata` stays a view of frontmatter.
- `search` keeps matching comment text, because a comment lives in the body.

## Lint
One new `Code::Comment`, with a span and a message for each failure:
- an entry heading that does not parse,
- an entry heading with no quote below it,
- a quote with no entry heading above it,
- a second `## Comments` section,
- a `## Comments` section that is not last.

No `--fix` for any of them. Every possible fix rewrites what a person wrote in an append-only log.

## Web (read-only)
- A thread under the body in the task detail view: open, not collapsed.
- One entry for each comment: the absolute timestamp, the author, and a badge for the agent.
- A damaged entry shows its text next to the field error.
- The thread refreshes on the existing `TaskChanged` event.
- No input box, and no comment column in the task table.

## Code placement
- `op-md`: append a block under a heading, create the heading at the end when it is absent, and stop
  reporting a heading that sits inside a blockquote. This is pure markdown work, and
  [[./00007-section-level-markdown-editing-o.md]] reuses the primitive.
- `op-task`: comment parse, serialize, and `append_comment` on top of the `op-md` primitive.
- `op-api`: `Comment` and the grouped branch shape.
- `op-server`: the two routes, the `TaskDetail` change, and the count.
- `op-cli`: the two commands and `author.rs` for the identity detection. A crate for identity would
  be ceremony while one module in one binary is the only consumer.
- `op-lint`: `Code::Comment`.
- Tests go in each crate's `tests/` directory.

## Acceptance criteria
- [x] `openplan comment OPP-1 "hello"` appends an entry and creates `## Comments` when it is absent.
- [x] The entry heading reads `### <timestamp> by <author> via <agent>` under an agent, and
      `### <timestamp> by <author>` under a person, and the text follows as a blockquote.
- [x] A comment that holds headings of every level, a fence, and a nested quote round-trips through a
      write and a read unchanged, and stays one entry.
- [x] `op_md::headings` ignores a heading inside a blockquote.
- [x] A second comment appends below the first and never reorders the file.
- [x] `openplan comment` fails with a clear error when git `user.name` is unset.
- [x] `openplan comment` refuses empty or whitespace-only text.
- [x] `openplan comments OPP-1` prints every entry, oldest first; `--json` prints the four fields.
- [x] `openplan comments --branch` and `--all-branches` read other branches, and `--all-branches`
      labels each entry with its branch.
- [x] A comment on a task that is not on the caller's branch is refused, and the error names the
      branches the task lives on.
- [x] A damaged entry heading parses into `Field::Error` with the offending text in the message, and
      the read still returns the text.
- [x] Section addressing offers no target inside `## Comments`, and a reference that names an entry
      heading does not resolve.
- [x] `openplan lint` reports each of the five comment failures with a span, and `--fix` changes
      nothing in a `## Comments` section.
- [x] `TaskDetail.body` excludes the `## Comments` section, and `TaskDetail.comments` holds it.
- [x] The web task detail shows the thread under the body, with an agent badge and an error mark on
      a damaged entry, and it refreshes when the file changes.
- [x] `cargo test`, `cargo fmt --check`, and `cargo clippy -- -D warnings` pass.

## Out of scope
- Editing or deleting a comment.
- Threads, replies, and comment ids.
- A comment anchored to a section.
- A merge-driver rule for the `## Comments` section.
- Writing a comment from the web UI.
