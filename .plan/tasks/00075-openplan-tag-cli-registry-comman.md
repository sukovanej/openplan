---
status: in_review
created: 2026-07-31T14:08:22Z
parent: ./00012-tags-registered-labels-name-colo.md
dependencies:
- ./00073-tags-wire-surface-op-api-dtos-ap.md
---
# openplan tag CLI: registry commands and --tag assignment through the daemon

Phase 5 of [[./00012-tags-registered-labels-name-colo.md]]: the CLI. Reads stay local; writes go through the daemon like every task write (`Writer` → `op_client`, `crates/op-cli/src/writer.rs:15-73`) — so `op-client` grows tag methods first. Consequence, same as all writes since the daemon became the write authority: `openplan tag create` needs a runnable daemon.

## Scope
- `op-client`: `create_tag` / `patch_tag` / `delete_tag` against `/api/tags`; `Writer` passthroughs.
- clap (`crates/op-cli/src/main.rs:37-143`): a `Tag` subcommand group — `create "<name>" [--color <c>] [--desc <text>]` (prints the normalized name), `list [--json]`, `show <name> [--json]`, `set <name> color <c>` / `set <name> desc <text>`, `rename <old> <new>`, `delete <name> [--force] [--yes]` (confirm like task `delete`, `:685-699`), `colors`. Reads (`list` / `show` / `colors`) hit the local `Store` like other reads.
- Assignment: `--tag <name>` (repeatable) on `Create` (`:40`); `parse_field` (`:652-676`) gains a `tags` arm — comma-split like `dependencies`, empty string clears — and the unknown-field message now lists it.
- Unknown-name rejections surface the server's `openplan tag create` hint and exit non-zero.

## Verify
`crates/op-cli/tests/cli.rs` on the `Project` fixture (`:29-58`): create/list/show/set/rename/delete/colors round-trip; duplicate and bad-color rejections; `create --tag` with an unknown name fails with the hint; `openplan set <id> tags "a, b"` writes a sorted set and `""` clears it; `delete` refuses referenced without `--force`.
