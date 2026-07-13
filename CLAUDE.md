# open-planner

Local-first task manager (Rust workspace). Design lives in `SPEC.md`, work in `.plan/tasks/`.

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

## Checks

```sh
cargo build
cargo test
cargo fmt --check
cargo clippy -- -D warnings
```
