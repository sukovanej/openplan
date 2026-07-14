import { expect, it } from "@effect/vitest"
import { Effect, Result } from "effect"
import { HttpClient, HttpClientResponse } from "effect/unstable/http"

import { ApiBaseUrl, getTask, listTasks, TaskNotFound } from "../src/lib/api"

const clientReturning = (response: () => Response) =>
  HttpClient.make((request) => Effect.succeed(HttpClientResponse.fromWeb(request, response())))

function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  })
}

const withResponse = (response: () => Response) => <A, E>(effect: Effect.Effect<A, E, HttpClient.HttpClient>) =>
  effect.pipe(
    Effect.provideService(HttpClient.HttpClient, clientReturning(response)),
    Effect.provideService(ApiBaseUrl, "http://localhost"),
  )

it.effect("decodes the branch-aware task list from GET /api/tasks", () =>
  withResponse(() =>
    json([
      {
        id: "a-1",
        title: "First",
        status: "todo",
        branches: [{ branch: "main", status: "todo", blob_oid: "aaa", dirty: false, kind: "base" }],
      },
      {
        id: "b-2",
        title: "Second",
        status: "in_progress",
        parent: "a-1",
        branches: [
          { branch: "main", status: "todo", blob_oid: "bbb", dirty: false, kind: "base" },
          { branch: "feature", status: "in_progress", blob_oid: "ccc", dirty: true, kind: "modified" },
        ],
      },
    ])
  )(
    Effect.gen(function*() {
      const tasks = yield* listTasks
      expect(tasks.map((t) => t.id)).toEqual(["a-1", "b-2"])
      expect(tasks[0].branches.map((b) => b.branch)).toEqual(["main"])
      expect(tasks[1].branches.length).toBe(2)
      expect(tasks[1].branches[1].dirty).toBe(true)
    }),
  ))

it.effect("decodes a single task view from GET /api/tasks/:id", () =>
  withResponse(() => json({ id: "a-1", title: "First", status: "todo", body: "# First\n" }))(
    Effect.gen(function*() {
      const task = yield* getTask("a-1")
      expect(task.title).toBe("First")
      expect(task.status).toBe("todo")
      expect(task.body).toBe("# First\n")
    }),
  ))

it.effect("maps a 404 response to TaskNotFound", () =>
  withResponse(() => json("no such task", 404))(
    Effect.gen(function*() {
      const failure = yield* Effect.flip(getTask("ghost"))
      expect(failure).toBeInstanceOf(TaskNotFound)
      expect((failure as TaskNotFound).id).toBe("ghost")
    }),
  ))

it.effect("rejects a malformed status with a decode failure", () =>
  withResponse(() => json([{ id: "a-1", title: "First", status: "bogus", branches: [] }]))(
    Effect.gen(function*() {
      const outcome = yield* Effect.result(listTasks)
      expect(Result.isFailure(outcome)).toBe(true)
    }),
  ))
