---
status: backlog
created: 2026-08-02T13:56:41Z
parent: ./00079-multi-repo-daemon-one-daemon-and.md
dependencies:
- ./00082-cli-oplan-project-commands-and-a.md
- ./00083-merged-all-projects-board.md
---
# SPA: project routes, switcher, merged home

Rewire the SPA onto the project routes. Give it the multi-project surface: a
merged home, a per-project view, and a switcher. This child also deletes the
daemon's unprefixed single-project routes. They lose their last callers here.

## Routes

`main.tsx` gets one more level:

- `/` — the merged all-projects board (`GET /api/board`).
- `/:project` — the board of that project.
- The task detail route gets the project segment. Put the segment in
  `TASK_ROUTE` and the route helper from
  [[./00066-spell-the-task-route-once-with-a.md]]. The segment is then
  spelled once.

A task link on the merged board carries the row's `project`. In a project
view, the segment comes from the URL.

## Project state

- `lib/abbreviation.ts` holds one global `AbbreviationStore`. Make it a
  per-project map. Load the map from `GET /api/projects`; this replaces
  `/api/config`. Key rendering resolves through the task's project.
- `lib/store.ts`: add the project to the query keys. One `boardQuery` per
  project, plus one merged. `taskQuery(project, id, branch)`.
- `lib/realtime.ts`: keep one EventSource. `applyChange` routes on the
  event's `project`: invalidate that project's queries and the merged board.
  On `ProjectsChanged`, read the project list again. On `Resync`, invalidate
  everything.

## Surface

- A header switcher: the merged view, plus one entry per project. Mark a
  demoted project and make its reason available (from `/api/projects`). Do
  not hide it.
- On the merged board, show the project name adjacent to the key on each
  row. This is the display answer to two stores that share an abbreviation.
- With zero projects, show an empty state that points at `oplan project add`.

## Cleanup

Delete the daemon's unprefixed `/api/tasks`, `/api/tasks/{id}`, and
`/api/config` delegations. The SPA was their last caller. The unprefixed
`/api/board` stays as the merged board. Delete `AppState::default_project`
(`op-server/src/lib.rs`) and the `serve.rs` wiring that passes the `--root`
project first. `--root` then changes nothing for `server start`, exactly as it
changes nothing for `server stop` today. Run `mise run generate-web-client`,
then `mise run rebuild`. The embedded SPA and the daemon then move together.

## Acceptance

- With two registered projects: the merged home shows both; the switcher
  moves between them; a detail opened from the merged board deep-links with
  the project in the URL; an SSE change in one project does not refetch the
  queries of the other project.
- With one project, the UI is today's UI plus the switcher chrome.
- `pnpm typecheck`, `pnpm lint`, `pnpm test`, and `pnpm format:check` pass.
  An interactive pass covers the merged board, the project board, the
  detail view, and the demoted-project mark.
