---
status: backlog
created: 2026-08-02T13:56:41Z
parent: ./00079-multi-repo-daemon-one-daemon-and.md
dependencies:
- ./00082-cli-oplan-project-commands-and-a.md
- ./00083-merged-all-projects-board.md
---
# SPA: project routes, switcher, merged home

Rewire the SPA onto the project routes and give it the multi-project surface:
a merged home, a per-project view, and a switcher. This child also deletes the
daemon's unprefixed single-project routes, which stop having callers here.

## Routes

`main.tsx` grows one level:

- `/` — the merged all-projects board (`GET /api/board`).
- `/:project` — that project's board.
- the task detail route gains the project segment; `TASK_ROUTE` and the route
  helper from [[./00066-spell-the-task-route-once-with-a.md]] absorb it so the
  segment is spelled once.

Task links rendered on the merged board carry the row's `project`; within a
project view the segment comes from the URL.

## Project state

- `lib/abbreviation.ts`'s single global `AbbreviationStore` becomes a
  per-project map loaded from `GET /api/projects` (replacing `/api/config`);
  key rendering resolves through the task's project.
- `lib/store.ts`: query keys gain the project — `boardQuery` per project plus
  one merged, `taskQuery(project, id, branch)`.
- `lib/realtime.ts`: still one EventSource. `applyChange` routes on the
  event's `project` — invalidate that project's queries and the merged board;
  `ProjectsChanged` re-reads the project list; `Resync` invalidates
  everything.

## Surface

- A header switcher: the merged view plus one entry per project, with a
  demoted project visibly marked and its reason available (from
  `/api/projects`), not silently absent.
- On the merged board, a task row shows its project name beside the key —
  the display answer to two stores sharing an abbreviation.
- Zero projects renders an empty state pointing at `oplan project add`.

## Cleanup

Delete the daemon's unprefixed `/api/tasks`, `/api/tasks/{id}`, and
`/api/config` delegations — the SPA was their last caller; the unprefixed
`/api/board` stays as the merged board. Regenerate the client
(`mise run generate-web-client`), then `mise run rebuild` so the embedded SPA
and the daemon move together.

## Acceptance

- With two registered projects: the merged home shows both, the switcher moves
  between them, a detail opened from the merged board deep-links with the
  project in the URL, and an SSE change in one project does not refetch the
  other's queries.
- With one project the UI is today's UI plus the switcher chrome.
- `pnpm typecheck`, `pnpm lint`, `pnpm test`, `pnpm format:check` pass, and an
  interactive pass covers the merged board, project board, detail, and the
  demoted-project marking.
