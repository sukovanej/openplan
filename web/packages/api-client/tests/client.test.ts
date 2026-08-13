import { expect, it } from "@effect/vitest"
import { Effect } from "effect"
import { HttpClient, HttpClientRequest, HttpClientResponse } from "effect/unstable/http"

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
            project: "open-plan",
            id: "a-1",
            title: "First",
            metadata: { status: "todo", created: "2026-01-01T00:00:00Z", parent: null, rank: null, dependencies: [] },
            updated: "2026-01-02T00:00:00Z",
            headline: "main",
            branches: [{ branch: "main", status: "todo", blob_oid: "aaa", dirty: false, kind: "base" }],
          },
        ]),
      ),
    )
    const result = yield* tasks.listTasks("open-plan", undefined)
    expect(result.map((t) => t.id)).toEqual(["a-1"])
    expect(result[0].branches[0].kind).toBe("base")
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
                    project: "open-plan",
                    id: "epic-1",
                    title: "Epic",
                    metadata: {
                      status: "todo",
                      created: "2026-01-01T00:00:00Z",
                      parent: null,
                      rank: null,
                      dependencies: [],
                    },
                    updated: "2026-01-02T00:00:00Z",
                    headline: "main",
                    branches: [],
                  },
                  depth: 0,
                  has_children: true,
                },
                {
                  task: {
                    project: "open-plan",
                    id: "kid-1",
                    title: "Kid",
                    metadata: {
                      status: "todo",
                      created: "2026-01-01T00:00:00Z",
                      parent: "epic-1",
                      rank: "m",
                      dependencies: [],
                    },
                    updated: "2026-01-02T00:00:00Z",
                    headline: "main",
                    branches: [],
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
    const board = yield* tasks.getBoard("open-plan", undefined)
    expect(board.groups.map((group) => group.status)).toEqual(["todo"])
    expect(board.groups[0].rows.map((row) => row.depth)).toEqual([0, 1])
    expect(board.groups[0].rows[1].task.metadata).toMatchObject({ parent: "epic-1" })
  }),
)

it.effect("decodes a branch-aware TaskDetail from GET /api/projects/:project/tasks/:id", () =>
  Effect.gen(function* () {
    const tasks = make(
      clientReturning(() =>
        json({
          project: "open-plan",
          id: "a-1",
          title: "First",
          metadata: {
            status: "in_progress",
            created: "2026-01-01T00:00:00Z",
            parent: null,
            rank: null,
            dependencies: ["2"],
          },
          body: "# First",
          updated: "2026-01-02T00:00:00Z",
          headline: "feature",
          branches: [
            { branch: "main", status: "todo", blob_oid: "bbb", dirty: false, kind: "base" },
            { branch: "feature", status: "in_progress", blob_oid: "ccc", dirty: true, kind: "modified" },
          ],
        }),
      ),
    )
    const detail = yield* tasks.getTask("open-plan", "a-1", { params: { branch: "feature" } })
    expect(detail.headline).toBe("feature")
    expect(detail.metadata).toMatchObject({ dependencies: ["2"] })
    expect(detail.branches.map((b) => b.branch)).toEqual(["main", "feature"])
  }),
)
