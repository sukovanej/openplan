import { expect, it } from "@effect/vitest"
import { Effect, Result } from "effect"
import { HttpClient, HttpClientRequest, HttpClientResponse, UrlParams } from "effect/unstable/http"

import { ApiBaseUrl, createTask, getTask, listTasks, patchTask, TaskNotFound, TaskRejected } from "../src/lib/api"

function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  })
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

const withResponse = (response: () => Response) => captureRequest(response).provide

function requestBody(request: HttpClientRequest.HttpClientRequest): unknown {
  const body = request.body
  if (body._tag === "Uint8Array") {
    return JSON.parse(new TextDecoder().decode(body.body))
  }
  return undefined
}

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
    ]),
  )(
    Effect.gen(function* () {
      const tasks = yield* listTasks
      expect(tasks.map((t) => t.id)).toEqual(["a-1", "b-2"])
      expect(tasks[0].branches.map((b) => b.branch)).toEqual(["main"])
      expect(tasks[1].headline).toBe("feature")
      expect(tasks[1].branches.length).toBe(2)
      expect(tasks[1].branches[1].dirty).toBe(true)
    }),
  ),
)

it.effect("decodes a task detail with its branch set and hierarchy from GET /api/tasks/:id", () =>
  withResponse(() =>
    json({
      id: "a-1",
      title: "First",
      status: "todo",
      parent: "epic-1",
      body: "# First\n",
      headline: "feature",
      branches: [
        { branch: "feature", status: "done", blob_oid: "ccc", dirty: false, kind: "modified" },
        { branch: "main", status: "todo", blob_oid: "aaa", dirty: false, kind: "base" },
      ],
      parent_title: "Epic",
      children: [{ id: "kid-1", title: "Kid", status: "in_progress", rank: "m" }],
      refs: [{ id: "epic-1", title: "Epic", status: "todo" }],
    }),
  )(
    Effect.gen(function* () {
      const task = yield* getTask("a-1")
      expect(task.title).toBe("First")
      expect(task.status).toBe("todo")
      expect(task.body).toBe("# First\n")
      expect(task.headline).toBe("feature")
      expect(task.branches.map((b) => b.branch)).toEqual(["feature", "main"])
      expect(task.parent_title).toBe("Epic")
      expect(task.children?.map((c) => c.id)).toEqual(["kid-1"])
      expect(task.refs?.[0].title).toBe("Epic")
    }),
  ),
)

it.effect("decodes a task detail that omits the optional hierarchy fields", () =>
  withResponse(() =>
    json({
      id: "solo-1",
      title: "Solo",
      status: "todo",
      body: "# Solo\n",
      headline: "main",
      branches: [],
    }),
  )(
    Effect.gen(function* () {
      const task = yield* getTask("solo-1")
      expect(task.parent_title).toBeUndefined()
      expect(task.children).toBeUndefined()
      expect(task.refs).toBeUndefined()
    }),
  ),
)

const detailResponse = () =>
  json({ id: "a-1", title: "First", status: "done", body: "# First\n", headline: "feature", branches: [] })

it.effect("requests a specific branch's version with ?branch=", () =>
  Effect.gen(function* () {
    const { captured, provide } = captureRequest(detailResponse)
    const task = yield* provide(getTask("a-1", "feature"))
    expect(task.status).toBe("done")
    expect(captured.request?.url).toContain("/api/tasks/a-1")
    expect(UrlParams.toString(captured.request!.urlParams)).toBe("branch=feature")
  }),
)

it.effect("omits the query entirely for a headline read", () =>
  Effect.gen(function* () {
    const { captured, provide } = captureRequest(detailResponse)
    yield* provide(getTask("a-1"))
    expect(UrlParams.toString(captured.request!.urlParams)).toBe("")
  }),
)

it.effect("maps a 404 response to TaskNotFound", () =>
  withResponse(() => json({ message: "no such task: ghost" }, 404))(
    Effect.gen(function* () {
      const failure = yield* Effect.flip(getTask("ghost"))
      expect(failure).toBeInstanceOf(TaskNotFound)
      expect((failure as TaskNotFound).id).toBe("ghost")
    }),
  ),
)

// The status decides what happened, not the body: a 404 from something other than the daemon (a
// proxy's error page, a bodyless response) must still reach the not-found view.
const foreign404 = {
  "a plain-text body": () => new Response("no such task", { status: 404 }),
  "an HTML body": () =>
    new Response("<html>Not Found</html>", { status: 404, headers: { "content-type": "text/html" } }),
  "a bodyless response": () => new Response(null, { status: 404 }),
}

for (const [shape, response] of Object.entries(foreign404)) {
  it.effect(`maps a 404 with ${shape} to TaskNotFound`, () =>
    withResponse(response)(
      Effect.gen(function* () {
        const failure = yield* Effect.flip(getTask("ghost"))
        expect(failure).toBeInstanceOf(TaskNotFound)
        expect((failure as TaskNotFound).id).toBe("ghost")
      }),
    ),
  )
}

it.effect("a read that fails on a malformed task carries the server's reason", () =>
  withResponse(() => json({ message: "invalid frontmatter in ghost.md" }, 500))(
    Effect.gen(function* () {
      const failure = yield* Effect.flip(getTask("ghost"))
      expect(failure).toBeInstanceOf(TaskRejected)
      expect((failure as TaskRejected).status).toBe(500)
      expect((failure as TaskRejected).message).toContain("invalid frontmatter")
    }),
  ),
)

it.effect("rejects a malformed status with a decode failure", () =>
  withResponse(() => json([{ id: "a-1", title: "First", status: "bogus", headline: "main", branches: [] }]))(
    Effect.gen(function* () {
      const outcome = yield* Effect.result(listTasks)
      expect(Result.isFailure(outcome)).toBe(true)
    }),
  ),
)

it.effect("PATCH sends parent: null to unparent and decodes the detail", () =>
  Effect.gen(function* () {
    const { captured, provide } = captureRequest(() =>
      json({ id: "child", title: "Child", status: "todo", body: "# Child\n", headline: "main", branches: [] }),
    )
    const detail = yield* provide(patchTask("child", { parent: null }))
    expect(captured.request?.method).toBe("PATCH")
    expect(captured.request?.url).toContain("/api/tasks/child")
    expect(requestBody(captured.request!)).toEqual({ parent: null })
    expect(detail.id).toBe("child")
  }),
)

it.effect("PATCH sends a parent id to reparent", () =>
  Effect.gen(function* () {
    const { captured, provide } = captureRequest(() =>
      json({ id: "child", title: "Child", status: "todo", body: "# Child\n", headline: "main", branches: [] }),
    )
    yield* provide(patchTask("child", { parent: "epic-1" }))
    expect(requestBody(captured.request!)).toEqual({ parent: "epic-1" })
  }),
)

it.effect("POST creates a child under a parent and returns the new id", () =>
  Effect.gen(function* () {
    const { captured, provide } = captureRequest(() => json({ id: "new-1" }, 201))
    const id = yield* provide(createTask({ title: "Subtask", parent: "root" }))
    expect(captured.request?.method).toBe("POST")
    expect(captured.request?.url).toContain("/api/tasks")
    expect(requestBody(captured.request!)).toEqual({ title: "Subtask", parent: "root" })
    expect(id).toBe("new-1")
  }),
)

it.effect("a refused PATCH carries the server's reason, not just a status code", () =>
  Effect.gen(function* () {
    const { provide } = captureRequest(() =>
      json({ message: "cannot reparent epic under its own descendant child" }, 400),
    )
    const result = yield* Effect.result(provide(patchTask("epic", { parent: "child" })))
    expect(Result.isFailure(result)).toBe(true)
    const error = Result.isFailure(result) ? result.failure : undefined
    expect(error).toBeInstanceOf(TaskRejected)
    expect((error as TaskRejected).status).toBe(400)
    expect((error as TaskRejected).message).toContain("own descendant")
  }),
)

it.effect("a PATCH aimed at a branch no worktree holds carries the 409 reason", () =>
  Effect.gen(function* () {
    const { provide } = captureRequest(() =>
      json({ message: "branch feature is not checked out in a writable worktree" }, 409),
    )
    const result = yield* Effect.result(provide(patchTask("a-1", { status: "done" })))
    const error = Result.isFailure(result) ? result.failure : undefined
    expect(error).toBeInstanceOf(TaskRejected)
    expect((error as TaskRejected).status).toBe(409)
    expect((error as TaskRejected).message).toContain("writable worktree")
  }),
)

it.effect("a write that fails inside the store carries the server's reason", () =>
  Effect.gen(function* () {
    const { provide } = captureRequest(() => json({ message: "permission denied writing epic.md" }, 500))
    const result = yield* Effect.result(provide(patchTask("epic", { parent: "child" })))
    const error = Result.isFailure(result) ? result.failure : undefined
    expect(error).toBeInstanceOf(TaskRejected)
    expect((error as TaskRejected).status).toBe(500)
    expect((error as TaskRejected).message).toContain("permission denied")
  }),
)

it.effect("a refused PATCH with no JSON body still fails with a readable reason", () =>
  Effect.gen(function* () {
    const { provide } = captureRequest(() => new Response("boom", { status: 500 }))
    const result = yield* Effect.result(provide(patchTask("epic", { parent: "child" })))
    expect(Result.isFailure(result)).toBe(true)
    const error = Result.isFailure(result) ? result.failure : undefined
    expect(error).toBeInstanceOf(TaskRejected)
    expect((error as TaskRejected).message).toContain("500")
  }),
)

// A status no route documents reaches the UI as the one fact worth reporting.
it.effect("an undocumented failure status still fails with its status", () =>
  Effect.gen(function* () {
    const { provide } = captureRequest(() => new Response("<html>Bad Gateway</html>", { status: 502 }))
    const result = yield* Effect.result(provide(patchTask("epic", { parent: "child" })))
    const error = Result.isFailure(result) ? result.failure : undefined
    expect(error).toBeInstanceOf(TaskRejected)
    expect((error as TaskRejected).status).toBe(502)
  }),
)

it.effect("a refused POST carries the server's reason", () =>
  Effect.gen(function* () {
    const { provide } = captureRequest(() => json({ message: "parent nope does not exist" }, 400))
    const result = yield* Effect.result(provide(createTask({ title: "Subtask", parent: "nope" })))
    expect(Result.isFailure(result)).toBe(true)
    const error = Result.isFailure(result) ? result.failure : undefined
    expect((error as TaskRejected).message).toContain("does not exist")
  }),
)
