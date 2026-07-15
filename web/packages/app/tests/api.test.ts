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
        headline: "main",
        branches: [{ branch: "main", status: "todo", blob_oid: "aaa", dirty: false, kind: "base" }],
      },
      {
        id: "b-2",
        title: "Second",
        status: "in_progress",
        parent: "a-1",
        headline: "feature",
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
      expect(tasks[1].headline).toBe("feature")
      expect(tasks[1].branches.length).toBe(2)
      expect(tasks[1].branches[1].dirty).toBe(true)
    }),
  ))

it.effect("decodes a task detail with its branch set from GET /api/tasks/:id", () =>
  withResponse(() =>
    json({
      id: "a-1",
      title: "First",
      status: "todo",
      body: "# First\n",
      headline: "feature",
      branches: [
        { branch: "feature", status: "done", blob_oid: "ccc", dirty: false, kind: "modified" },
        { branch: "main", status: "todo", blob_oid: "aaa", dirty: false, kind: "base" },
      ],
    })
  )(
    Effect.gen(function*() {
      const task = yield* getTask("a-1")
      expect(task.title).toBe("First")
      expect(task.status).toBe("todo")
      expect(task.body).toBe("# First\n")
      expect(task.headline).toBe("feature")
      expect(task.branches.map((b) => b.branch)).toEqual(["feature", "main"])
    }),
  ))

it.effect("requests a specific branch's version with ?branch=", () =>
  Effect.gen(function*() {
    let requested: string | undefined
    const client = HttpClient.make((request) => {
      requested = request.url
      return Effect.succeed(
        HttpClientResponse.fromWeb(
          request,
          json({
            id: "a-1",
            title: "First",
            status: "done",
            body: "# First\n",
            headline: "feature",
            branches: [],
          }),
        ),
      )
    })
    const task = yield* getTask("a-1", "feature").pipe(
      Effect.provideService(HttpClient.HttpClient, client),
      Effect.provideService(ApiBaseUrl, "http://localhost"),
    )
    expect(task.status).toBe("done")
    expect(requested).toContain("/api/tasks/a-1?branch=feature")
  }))

it.effect("maps a 404 response to TaskNotFound", () =>
  withResponse(() => json("no such task", 404))(
    Effect.gen(function*() {
      const failure = yield* Effect.flip(getTask("ghost"))
      expect(failure).toBeInstanceOf(TaskNotFound)
      expect((failure as TaskNotFound).id).toBe("ghost")
    }),
  ))

it.effect("rejects a malformed status with a decode failure", () =>
  withResponse(() => json([{ id: "a-1", title: "First", status: "bogus", headline: "main", branches: [] }]))(
    Effect.gen(function*() {
      const outcome = yield* Effect.result(listTasks)
      expect(Result.isFailure(outcome)).toBe(true)
    }),
  ))

import { HttpClientRequest } from "effect/unstable/http"
import { createTask, getTree, patchTask } from "../src/lib/api"

function decodeBody(request: HttpClientRequest.HttpClientRequest): unknown {
  const body = request.body
  if (body._tag === "Uint8Array") {
    return JSON.parse(new TextDecoder().decode(body.body))
  }
  return undefined
}

const captureRequest = (response: () => Response) => {
  const captured: { request?: HttpClientRequest.HttpClientRequest } = {}
  const client = HttpClient.make((request) => {
    captured.request = request
    return Effect.succeed(HttpClientResponse.fromWeb(request, response()))
  })
  const provide = <A, E>(effect: Effect.Effect<A, E, HttpClient.HttpClient>) =>
    effect.pipe(
      Effect.provideService(HttpClient.HttpClient, client),
      Effect.provideService(ApiBaseUrl, "http://localhost"),
    )
  return { captured, provide }
}

it.effect("decodes a nested subtree from GET /api/tasks/:id/tree", () =>
  withResponse(() =>
    json({
      id: "root",
      title: "Root",
      status: "todo",
      children: [
        {
          id: "child",
          title: "Child",
          status: "in_progress",
          parent: "root",
          rank: "m",
          children: [
            { id: "grand", title: "Grand", status: "done", parent: "child", rank: "m", children: [] },
          ],
        },
      ],
    })
  )(
    Effect.gen(function*() {
      const tree = yield* getTree("root")
      expect(tree.children.map((c) => c.id)).toEqual(["child"])
      expect(tree.children[0].children[0].id).toBe("grand")
    }),
  ))

it.effect("bounds the tree request with ?depth=", () =>
  Effect.gen(function*() {
    const { captured, provide } = captureRequest(() =>
      json({ id: "root", title: "Root", status: "todo", children: [] })
    )
    yield* provide(getTree("root", 1))
    expect(captured.request?.url).toContain("/api/tasks/root/tree?depth=1")
  }))

it.effect("PATCH sends parent: null to unparent and decodes the detail", () =>
  Effect.gen(function*() {
    const { captured, provide } = captureRequest(() =>
      json({ id: "child", title: "Child", status: "todo", body: "# Child\n", headline: "main", branches: [] })
    )
    const detail = yield* provide(patchTask("child", { parent: null }))
    expect(captured.request?.method).toBe("PATCH")
    expect(captured.request?.url).toContain("/api/tasks/child")
    expect(decodeBody(captured.request!)).toEqual({ parent: null })
    expect(detail.id).toBe("child")
  }))

it.effect("PATCH sends a parent id to reparent", () =>
  Effect.gen(function*() {
    const { captured, provide } = captureRequest(() =>
      json({ id: "child", title: "Child", status: "todo", body: "# Child\n", headline: "main", branches: [] })
    )
    yield* provide(patchTask("child", { parent: "epic-1" }))
    expect(decodeBody(captured.request!)).toEqual({ parent: "epic-1" })
  }))

it.effect("POST creates a child under a parent and returns the new id", () =>
  Effect.gen(function*() {
    const { captured, provide } = captureRequest(() => json({ id: "new-1" }, 201))
    const id = yield* provide(createTask({ title: "Subtask", parent: "root" }))
    expect(captured.request?.method).toBe("POST")
    expect(captured.request?.url).toContain("/api/tasks")
    expect(decodeBody(captured.request!)).toEqual({ title: "Subtask", parent: "root" })
    expect(id).toBe("new-1")
  }))
