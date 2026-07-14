---
status: done
---
# Support setting task content on create via --body / --body-file

`oplan create "<title>"` currently produces a body of only `# <title>`. There is
no way to set the markdown content below the heading at creation time. Add flags
to specify it, using the `gh issue create` convention that agents reach for by
reflex.

## Interface

- `--body <text>` — inline content.
- `--body-file <path>` — read content from a file; `-` means stdin.
- The two flags are mutually exclusive (error if both are given).
- Content is placed below the `# <title>` heading so `Task::title()` keeps
  reading the title from the first `#` line.

```sh
oplan create "Ship login" --body "Support OAuth and email login."
oplan create "Ship login" --body-file notes.md
oplan create "Ship login" --body-file - <<'EOF'
## Goals
- OAuth
- Email + password
EOF
```

## Implementation

- `op-cli` `Command::Create`: add `body: Option<String>` and `body_file:
  Option<String>` args; resolve to an optional content string (reading the file
  or stdin for `body_file`, `-` = stdin); reject when both are set.
- `op-api` `CreateTask`: add `body: Option<String>` field; `into_task` appends it
  below the title heading. This keeps the daemon/HTTP create path in sync with
  the CLI rather than special-casing content in the CLI only.
- `op-task` `Task::new`: keep the title-only constructor; add the content by
  writing `# <title>\n\n<content>\n` (normalize a single trailing newline). Or
  give `Task` a helper that appends body content so the H1 invariant lives in
  one place.

## Tests

- `op-cli/tests/cli.rs`: create with `--body`, with `--body-file <path>`, with
  `--body-file -` (piped stdin); assert the stored file contains the heading then
  the content, and that `get --json` reports the expected `body`. Assert the
  `--body` + `--body-file` conflict errors.
- `op-task`/`op-api`: `into_task` with and without body yields the expected body
  string and a correct `title()`.
