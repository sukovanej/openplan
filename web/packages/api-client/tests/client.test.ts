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
