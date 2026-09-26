---
status: backlog
created: 2026-07-26T15:27:08Z
---
# CI: run the web tests, fail on a stale web client, and keep one list of checks

## Goal

`.github/workflows/ci.yml` runs the Rust checks, the web lint and format checks,
and the SPA build. Three gaps remain. Close them.

## Gaps

1. **Web tests and types.** CI does not run `pnpm test` (vitest) or
   `pnpm typecheck`. The `tsc -b` in the app build checks only the app. It does
   not check the tests in `web/packages/ui/tests` and
   `web/packages/api-client/tests`. Add both commands to the `web` job.
2. **Stale web client.** No job regenerates
   `web/packages/api-client/src/index.ts` from the OpenAPI spec of the Rust API.
   A change to the API can land without a new client. In the `rust` job, after
   the build, run `mise run generate-web-client`. Then fail the job if a tracked
   file changed or a new untracked file appeared.
3. **Two lists of checks.** The README lists the cargo and pnpm checks, so a
   change to the workflow must also change the README. Remove the list from the
   README and refer to the workflow.

## Done when

- A pull request that breaks a web test or a web type is red.
- A pull request that changes the API and does not regenerate the client is red.
- The workflow is the only list of the checks in the README.
