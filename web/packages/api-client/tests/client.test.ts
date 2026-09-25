import { expect, it } from "@effect/vitest"
import { Effect } from "effect"
import { HttpClient, HttpClientRequest, HttpClientResponse, UrlParams } from "effect/unstable/http"

import { make } from "../src/index.ts"

const clientReturning = (response: () => Response) =>
  HttpClient.make((request) => Effect.succeed(HttpClientResponse.fromWeb(request, response()))).pipe(
    HttpClient.mapRequest(HttpClientRequest.prependUrl("http://localhost")),
  )

const json = (body: unknown, status = 200): Response =>
  new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  })

it.effect("decodes GET /api/projects/:project/tasks through the generated client", () =>
  Effect.gen(function* () {
    const tasks = make(
      clientReturning(() =>
        json([
          {
            project: "openplan",
            id: "a-1",
            title: "First",
            metadata: {
              status: "todo",
              created: "2026-01-01T00:00:00Z",
              parent: null,
              rank: null,
              dependencies: [],
              tags: [],
            },
            updated: "2026-01-02T00:00:00Z",
            comment_count: 2,
            conflicts: 0,
            problems: [],
          },
        ]),
      ),
    )
    const result = yield* tasks.listTasks("openplan", undefined)
    expect(result.map((t) => t.id)).toEqual(["a-1"])
    expect(result[0].comment_count).toBe(2)
  }),
)

it.effect("decodes the grouped, flattened board from GET /api/projects/:project/board", () =>
  Effect.gen(function* () {
    const tasks = make(
      clientReturning(() =>
        json({
          groups: [
            {
              status: "todo",
              rows: [
                {
                  task: {
                    project: "openplan",
                    id: "epic-1",
                    title: "Epic",
                    metadata: {
                      status: "todo",
                      created: "2026-01-01T00:00:00Z",
                      parent: null,
                      rank: null,
                      dependencies: [],
                      tags: [],
                    },
                    updated: "2026-01-02T00:00:00Z",
                    comment_count: 0,
                    conflicts: 0,
                    problems: [],
                  },
                  depth: 0,
                  has_children: true,
                },
                {
                  task: {
                    project: "openplan",
                    id: "kid-1",
                    title: "Kid",
                    metadata: {
                      status: "todo",
                      created: "2026-01-01T00:00:00Z",
                      parent: "epic-1",
                      rank: "m",
                      dependencies: [],
                      tags: [],
                    },
                    updated: "2026-01-02T00:00:00Z",
                    comment_count: 0,
                    conflicts: 0,
                    problems: [],
                  },
                  depth: 1,
                  has_children: false,
                },
              ],
            },
          ],
        }),
      ),
    )
    const board = yield* tasks.getBoard("openplan", undefined)
    expect(board.groups.map((group) => group.status)).toEqual(["todo"])
    expect(board.groups[0].rows.map((row) => row.depth)).toEqual([0, 1])
    expect(board.groups[0].rows[1].task.metadata).toMatchObject({ parent: "epic-1" })
  }),
)

it.effect("decodes a TaskDetail from GET /api/projects/:project/tasks/:id", () =>
  Effect.gen(function* () {
    const tasks = make(
      clientReturning(() =>
        json({
          project: "openplan",
          id: "a-1",
          title: "First",
          metadata: {
            status: "in_progress",
            created: "2026-01-01T00:00:00Z",
            parent: null,
            rank: null,
            dependencies: ["2"],
            tags: [],
          },
          body: "# First",
          conflicts: 0,
          problems: [],
          updated: "2026-01-02T00:00:00Z",
        }),
      ),
    )
    const detail = yield* tasks.getTask("openplan", "a-1", undefined)
    expect(detail.metadata).toMatchObject({ status: "in_progress", dependencies: ["2"] })
    expect(detail.body).toBe("# First")
  }),
)

it.effect("decodes the revisions of a task, and pages with `before`", () =>
  Effect.gen(function* () {
    const urls: Array<string> = []
    const tasks = make(
      HttpClient.make((request) => {
        urls.push(`${request.url}?${UrlParams.toString(request.urlParams)}`)
        return Effect.succeed(
          HttpClientResponse.fromWeb(
            request,
            json([
              {
                revision: {
                  id: "c0ffee",
                  parents: ["beef"],
                  author: "Milan",
                  agent: "claude_code",
                  at: "2026-01-02T00:00:00Z",
                  message: "Set OPP-1 to done",
                },
                changes: [{ path: "tasks/00001-first.md", kind: "modified", task: "OPP-1" }],
                summary: ["OPP-1: status → done"],
                tasks: [
                  {
                    task: "OPP-1",
                    kind: "modified",
                    title: "First",
                    fields: [{ field: "status", from: "todo", to: "done" }],
                  },
                ],
                tags: [],
              },
            ]),
          ),
        )
      }).pipe(HttpClient.mapRequest(HttpClientRequest.prependUrl("http://localhost"))),
    )
    const history = yield* tasks.taskHistory("openplan", "OPP-1", { params: { before: "d00d", limit: 20 } })
    expect(urls).toEqual(["http://localhost/api/projects/openplan/tasks/OPP-1/history?before=d00d&limit=20"])
    expect(history[0].revision.agent).toBe("claude_code")
    expect(history[0].revision.email).toBeUndefined()
    expect(history[0].changes[0]).toEqual({ path: "tasks/00001-first.md", kind: "modified", task: "OPP-1" })
    expect(history[0].tasks[0].fields).toEqual([{ field: "status", from: "todo", to: "done" }])
  }),
)

it.effect("decodes a task at a revision where it did not exist", () =>
  Effect.gen(function* () {
    const tasks = make(clientReturning(() => json({ id: "OPP-1", revision: "c0ffee" })))
    const at = yield* tasks.taskRevision("openplan", "OPP-1", "c0ffee", undefined)
    expect(at.task).toBeUndefined()
  }),
)

it.effect("decodes the sync state of a project", () =>
  Effect.gen(function* () {
    const tasks = make(
      clientReturning(() =>
        json({ remote: "origin", last_attempt: "2026-01-02T00:00:00Z", ahead: 1, behind: 0, error: "no route" }),
      ),
    )
    const sync = yield* tasks.getSync("openplan", undefined)
    expect(sync).toEqual({
      remote: "origin",
      last_attempt: "2026-01-02T00:00:00Z",
      ahead: 1,
      behind: 0,
      error: "no route",
    })
  }),
)
