---
status: todo
---
# CI: run every check and assert the tree is unchanged

## Goal

The repo has no CI. Every check is currently a thing a human remembers to run,
and two of them are silent-drift risks: the generated web client
(`web/packages/api-client/src/index.ts`) can fall behind the Rust API, and
formatting can land unformatted. A CI job should run the full suite on every
push and pull request, and fail if any tracked file changed while it ran.

## Scope

- A GitHub Actions workflow (`.github/workflows/ci.yml`) triggered on `push` to
  `main` and on `pull_request`.
- Rust: `cargo build`, `cargo test`, `cargo fmt --check`,
  `cargo clippy -- -D warnings`.
- Web: `pnpm lint`, `pnpm format:check`, `pnpm typecheck`, `pnpm test`,
  `pnpm -r build`.
- Generation: `mise run generate-web-client`, then `git diff --exit-code` (plus
  a check for untracked files) so a client that no longer matches the Rust
  API's OpenAPI spec fails the build instead of drifting.
- Ordering: `op-server` embeds `web/packages/app/dist` via `rust-embed`, so the
  SPA has to be built before anything runs `cargo`. Whether that means one job
  in sequence or two jobs sharing an artifact is the design call to make here.
- Cache cargo and the pnpm store so the job stays quick.

## Done when

- A pull request that leaves the generated client stale, a file unformatted, or
  any check failing is red; a clean one is green.
- The workflow is the single place that lists the checks — no second copy of
  the command list in the README.
