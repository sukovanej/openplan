# open-planner

Local-first task manager (Rust workspace). Design and work live in `.plan/tasks/`.

## Worktrees

Never write to the main checkout. Every change to a tracked file goes in a
separate git worktree — source, docs, config, and `.plan/tasks/` task files
alike, including writes made through the `openplan` CLI. "It's one line", "it's
just a task", and "it's only the tracker, not code" are not exceptions.

Before the first write of any unit of work, confirm you are not in the primary
worktree (in it, `git rev-parse --git-dir` equals `--git-common-dir`); if you
are, create a dedicated worktree and switch into it first. Create one worktree
per unit of work and remove it once the change is merged.

## Language: ASD-STE100 Simplified Technical English

Write all prose in ASD-STE100 Simplified Technical English. This rule applies
to task files, documentation, commit messages, pull requests, and replies to
the user. Use the active voice. Write short sentences. Give one instruction in
each sentence. Use one meaning for each word.

## Style: minimal comments, docs, and README

Code must be self-describing. If a piece of code seems to need a comment, a doc
comment, or a README paragraph to be understood, treat that as a defect in the
code — improve names, types, and structure until the prose is unnecessary,
rather than writing the prose.

- No doc comments (`///`, `//!`) on self-describing items — which should be all of them.
- A plain comment is allowed only for *why* the code cannot express itself: a
  footgun, an external constraint, or a deliberate placeholder (`TODO(...)`).
  Never restate *what* the code does.
- The README stays at build/run essentials. A crate's purpose belongs in its
  `Cargo.toml` `description`, not in prose.
- Exempt, because they are product UI rather than documentation: CLI `--help`
  text (clap `///` / `about`) and user-facing error/log messages.

## Tests

Tests live in a crate's `tests/` directory, never inside `src/`. Do not add
`#[cfg(test)] mod tests` blocks or `#[test]` functions to source files. To
exercise crate-private items, widen their visibility (e.g. `pub(crate)`) so a
`tests/` file can reach them.

## Checks

```sh
cargo build
cargo test
cargo fmt --check
cargo clippy -- -D warnings
```

## Commands (mise)

Recurring workflows are `mise` tasks (see `mise.toml`) — run the named task, not
its underlying commands by hand. Before treating a bare verb like `rebuild` as a
cargo command, check whether it names a `mise` task and, if so, run that:

```sh
mise run rebuild               # rebuild the web SPA and restart the daemon on the fresh embed
mise run generate-web-client   # regenerate the Effect HTTP client from the API's OpenAPI spec
```
