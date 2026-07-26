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

it.effect("decodes GET /api/tasks through the generated client", () =>
  Effect.gen(function* () {
    const tasks = make(
      clientReturning(() =>
        json([
          {
            id: "a-1",
            title: "First",
            status: "todo",
            headline: "main",
            branches: [{ branch: "main", status: "todo", blob_oid: "aaa", dirty: false, kind: "base" }],
          },
        ]),
      ),
    )
    const result = yield* tasks.listTasks(undefined)
    expect(result.map((t) => t.id)).toEqual(["a-1"])
    expect(result[0].branches[0].kind).toBe("base")
  }),
)

it.effect("decodes the grouped, flattened board from GET /api/board", () =>
  Effect.gen(function* () {
    const tasks = make(
      clientReturning(() =>
        json({
          groups: [
            {
              status: "todo",
              rows: [
                {
                  task: { id: "epic-1", title: "Epic", status: "todo", headline: "main", branches: [] },
                  depth: 0,
                  has_children: true,
                },
                {
                  task: {
                    id: "kid-1",
                    title: "Kid",
                    status: "todo",
                    parent: "epic-1",
                    rank: "m",
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
    const board = yield* tasks.getBoard(undefined)
    expect(board.groups.map((group) => group.status)).toEqual(["todo"])
    expect(board.groups[0].rows.map((row) => row.depth)).toEqual([0, 1])
    expect(board.groups[0].rows[1].task.parent).toBe("epic-1")
  }),
)

it.effect("decodes a branch-aware TaskDetail from GET /api/tasks/:id", () =>
  Effect.gen(function* () {
    const tasks = make(
      clientReturning(() =>
        json({
          id: "a-1",
          title: "First",
          status: "in_progress",
          deps: ["b-2"],
          body: "# First",
          headline: "feature",
          branches: [
            { branch: "main", status: "todo", blob_oid: "bbb", dirty: false, kind: "base" },
            { branch: "feature", status: "in_progress", blob_oid: "ccc", dirty: true, kind: "modified" },
          ],
        }),
      ),
    )
    const detail = yield* tasks.getTask("a-1", { params: { branch: "feature" } })
    expect(detail.headline).toBe("feature")
    expect(detail.deps).toEqual(["b-2"])
    expect(detail.branches.map((b) => b.branch)).toEqual(["main", "feature"])
  }),
)
