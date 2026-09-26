import { expect, it } from "@effect/vitest"
import { Effect, Result } from "effect"
import { HttpClient, HttpClientRequest, HttpClientResponse, UrlParams } from "effect/unstable/http"

import {
  ApiBaseUrl,
  createTask,
  deleteTag,
  getProjectHistory,
  getTask,
  getTaskRevision,
  listTasks,
  patchTask,
  runSync,
  TaskRejected,
  writeTaskText,
  TaskNotFound,
} from "../src/lib/api"

const PROJECT = "openplan"

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

it.effect("decodes the task list from GET /api/projects/:project/tasks", () =>
  withResponse(() =>
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
        comment_count: 0,
        conflicts: 0,
        problems: [],
      },
      {
        project: "openplan",
        id: "b-2",
        title: "Second",
        metadata: {
          status: "in_progress",
          created: "2026-01-01T00:00:00Z",
          parent: "a-1",
          rank: null,
          dependencies: [],
          tags: [],
        },
        updated: "2026-01-02T00:00:00Z",
        comment_count: 3,
        conflicts: 0,
        problems: [],
      },
    ]),
  )(
    Effect.gen(function* () {
      const tasks = yield* listTasks(PROJECT)
      expect(tasks.map((t) => t.id)).toEqual(["a-1", "b-2"])
      expect(tasks[1].metadata).toMatchObject({ parent: "a-1" })
      expect(tasks[1].comment_count).toBe(3)
    }),
  ),
)

it.effect("decodes a task detail with its hierarchy from GET /api/projects/:project/tasks/:id", () =>
  withResponse(() =>
    json({
      project: "openplan",
      id: "a-1",
      title: "First",
      description: "See [[OPP-2]].\n",
      conflicts: 0,
      problems: [],
      metadata: {
        status: "todo",
        created: "2026-01-01T00:00:00Z",
        parent: "epic-1",
        rank: null,
        dependencies: [],
        tags: [],
      },
      updated: "2026-01-02T00:00:00Z",
      parent_title: "Epic",
      children: [{ id: "kid-1", title: "Kid", status: "in_progress", rank: "m" }],
      refs: [{ id: "epic-1", title: "Epic", status: "todo" }],
    }),
  )(
    Effect.gen(function* () {
      const task = yield* getTask(PROJECT, "a-1")
      expect(task.title).toBe("First")
      expect(task.metadata).toMatchObject({ status: "todo" })
      expect(task.description).toBe("See [[OPP-2]].\n")
      expect(task.parent_title).toBe("Epic")
      expect(task.children?.map((c) => c.id)).toEqual(["kid-1"])
      expect(task.refs?.[0].title).toBe("Epic")
    }),
  ),
)

it.effect("decodes a task detail that omits the optional hierarchy fields", () =>
  withResponse(() =>
    json({
      project: "openplan",
      id: "solo-1",
      title: "Solo",
      metadata: {
        status: "todo",
        created: "2026-01-01T00:00:00Z",
        parent: null,
        rank: null,
        dependencies: [],
        tags: [],
      },
      description: "",
      conflicts: 0,
      problems: [],
      updated: "2026-01-02T00:00:00Z",
    }),
  )(
    Effect.gen(function* () {
      const task = yield* getTask(PROJECT, "solo-1")
      expect(task.parent_title).toBeUndefined()
      expect(task.children).toBeUndefined()
      expect(task.refs).toBeUndefined()
    }),
  ),
)

it.effect("sends no query for a task read", () =>
  Effect.gen(function* () {
    const { captured, provide } = captureRequest(() =>
      json({
        project: "openplan",
        id: "a-1",
        title: "First",
        metadata: {
          status: "done",
          created: "2026-01-01T00:00:00Z",
          parent: null,
          rank: null,
          dependencies: [],
          tags: [],
        },
        description: "",
        conflicts: 0,
        problems: [],
        updated: "2026-01-02T00:00:00Z",
      }),
    )
    yield* provide(getTask(PROJECT, "a-1"))
    expect(captured.request?.url).toContain("/api/projects/openplan/tasks/a-1")
    expect(UrlParams.toString(captured.request!.urlParams)).toBe("")
  }),
)

it.effect("maps a 404 response to TaskNotFound", () =>
  withResponse(() => json({ message: "no such task: ghost" }, 404))(
    Effect.gen(function* () {
      const failure = yield* Effect.flip(getTask(PROJECT, "ghost"))
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
        const failure = yield* Effect.flip(getTask(PROJECT, "ghost"))
        expect(failure).toBeInstanceOf(TaskNotFound)
        expect((failure as TaskNotFound).id).toBe("ghost")
      }),
    ),
  )
}

it.effect("a read that fails on a malformed task carries the server's reason", () =>
  withResponse(() => json({ message: "invalid frontmatter in ghost.md" }, 500))(
    Effect.gen(function* () {
      const failure = yield* Effect.flip(getTask(PROJECT, "ghost"))
      expect(failure).toBeInstanceOf(TaskRejected)
      expect((failure as TaskRejected).status).toBe(500)
      expect((failure as TaskRejected).message).toContain("invalid frontmatter")
    }),
  ),
)

it.effect("rejects a malformed status with a decode failure", () =>
  withResponse(() =>
    json([
      {
        project: "openplan",
        id: "a-1",
        title: "First",
        metadata: {
          status: "bogus",
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
    ]),
  )(
    Effect.gen(function* () {
      const outcome = yield* Effect.result(listTasks(PROJECT))
      expect(Result.isFailure(outcome)).toBe(true)
    }),
  ),
)

it.effect("PATCH sends parent: null to unparent and decodes the detail", () =>
  Effect.gen(function* () {
    const { captured, provide } = captureRequest(() =>
      json({
        project: "openplan",
        id: "child",
        title: "Child",
        metadata: {
          status: "todo",
          created: "2026-01-01T00:00:00Z",
          parent: null,
          rank: null,
          dependencies: [],
          tags: [],
        },
        description: "",
        conflicts: 0,
        problems: [],
        updated: "2026-01-02T00:00:00Z",
      }),
    )
    const detail = yield* provide(patchTask(PROJECT, "child", { parent: null }))
    expect(captured.request?.method).toBe("PATCH")
    expect(captured.request?.url).toContain("/api/projects/openplan/tasks/child")
    expect(requestBody(captured.request!)).toEqual({ parent: null })
    expect(detail.id).toBe("child")
  }),
)

it.effect("PATCH sends a parent id to reparent", () =>
  Effect.gen(function* () {
    const { captured, provide } = captureRequest(() =>
      json({
        project: "openplan",
        id: "child",
        title: "Child",
        metadata: {
          status: "todo",
          created: "2026-01-01T00:00:00Z",
          parent: null,
          rank: null,
          dependencies: [],
          tags: [],
        },
        description: "",
        conflicts: 0,
        problems: [],
        updated: "2026-01-02T00:00:00Z",
      }),
    )
    yield* provide(patchTask(PROJECT, "child", { parent: "epic-1" }))
    expect(requestBody(captured.request!)).toEqual({ parent: "epic-1" })
  }),
)

it.effect("POST creates a child under a parent and returns the new id", () =>
  Effect.gen(function* () {
    const { captured, provide } = captureRequest(() => json({ id: "new-1" }, 201))
    const id = yield* provide(createTask(PROJECT, { title: "Subtask", parent: "root" }))
    expect(captured.request?.method).toBe("POST")
    expect(captured.request?.url).toContain("/api/projects/openplan/tasks")
    expect(requestBody(captured.request!)).toEqual({ title: "Subtask", parent: "root" })
    expect(id).toBe("new-1")
  }),
)

it.effect("a refused PATCH carries the server's reason, not just a status code", () =>
  Effect.gen(function* () {
    const { provide } = captureRequest(() =>
      json({ message: "cannot reparent epic under its own descendant child" }, 400),
    )
    const result = yield* Effect.result(provide(patchTask(PROJECT, "epic", { parent: "child" })))
    expect(Result.isFailure(result)).toBe(true)
    const error = Result.isFailure(result) ? result.failure : undefined
    expect(error).toBeInstanceOf(TaskRejected)
    expect((error as TaskRejected).status).toBe(400)
    expect((error as TaskRejected).message).toContain("own descendant")
  }),
)

it.effect("a PATCH that other writers keep racing carries the 409 reason", () =>
  Effect.gen(function* () {
    const { provide } = captureRequest(() => json({ message: "another writer kept moving the tasks" }, 409))
    const result = yield* Effect.result(provide(patchTask(PROJECT, "a-1", { status: "done" })))
    const error = Result.isFailure(result) ? result.failure : undefined
    expect(error).toBeInstanceOf(TaskRejected)
    expect((error as TaskRejected).status).toBe(409)
    expect((error as TaskRejected).message).toContain("another writer")
  }),
)

// One status covers several tag-delete refusals, and `force` answers only one of them. The field is
// what tells the caller which one it received.
it.effect("a tag delete a reference count refuses names the refusal force answers", () =>
  Effect.gen(function* () {
    const { provide } = captureRequest(() =>
      json({ message: "tag backend is used by 2 task(s)", reason: "tag_referenced" }, 409),
    )
    const result = yield* Effect.result(provide(deleteTag(PROJECT, "backend", false)))
    const error = Result.isFailure(result) ? result.failure : undefined
    expect((error as TaskRejected).status).toBe(409)
    expect((error as TaskRejected).reason).toBe("tag_referenced")
  }),
)

it.effect("encodes a path segment exactly once", () =>
  Effect.gen(function* () {
    const { captured, provide } = captureRequest(() => new Response(null, { status: 204 }))
    yield* provide(deleteTag(PROJECT, "area/web", false))
    expect(captured.request?.url).toContain("/api/projects/openplan/tags/area%2Fweb")
  }),
)

it.effect("a tag delete that other writers race names no refusal force answers", () =>
  Effect.gen(function* () {
    const { provide } = captureRequest(() => json({ message: "another writer kept moving the tasks" }, 409))
    const result = yield* Effect.result(provide(deleteTag(PROJECT, "backend", false)))
    const error = Result.isFailure(result) ? result.failure : undefined
    expect((error as TaskRejected).status).toBe(409)
    expect((error as TaskRejected).reason).toBeUndefined()
  }),
)

it.effect("a write that fails inside the store carries the server's reason", () =>
  Effect.gen(function* () {
    const { provide } = captureRequest(() => json({ message: "permission denied writing epic.md" }, 500))
    const result = yield* Effect.result(provide(patchTask(PROJECT, "epic", { parent: "child" })))
    const error = Result.isFailure(result) ? result.failure : undefined
    expect(error).toBeInstanceOf(TaskRejected)
    expect((error as TaskRejected).status).toBe(500)
    expect((error as TaskRejected).message).toContain("permission denied")
  }),
)

it.effect("a refused PATCH with no JSON body still fails with a readable reason", () =>
  Effect.gen(function* () {
    const { provide } = captureRequest(() => new Response("boom", { status: 500 }))
    const result = yield* Effect.result(provide(patchTask(PROJECT, "epic", { parent: "child" })))
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
    const result = yield* Effect.result(provide(patchTask(PROJECT, "epic", { parent: "child" })))
    const error = Result.isFailure(result) ? result.failure : undefined
    expect(error).toBeInstanceOf(TaskRejected)
    expect((error as TaskRejected).status).toBe(502)
  }),
)

it.effect("a refused POST carries the server's reason", () =>
  Effect.gen(function* () {
    const { provide } = captureRequest(() => json({ message: "parent nope does not exist" }, 400))
    const result = yield* Effect.result(provide(createTask(PROJECT, { title: "Subtask", parent: "nope" })))
    expect(Result.isFailure(result)).toBe(true)
    const error = Result.isFailure(result) ? result.failure : undefined
    expect((error as TaskRejected).message).toContain("does not exist")
  }),
)

const entry = (id: string, parents: ReadonlyArray<string>) => ({
  revision: { id, parents, author: "Milan", at: "2026-01-02T00:00:00Z", message: `Revision ${id}` },
  changes: [{ path: "tasks/00001-first.md", kind: "modified", task: "OPP-1" }],
  summary: ["OPP-1: description"],
  tasks: [{ task: "OPP-1", kind: "modified", title: "First", fields: [{ field: "description" }] }],
  tags: [],
  docs: [],
})

it.effect("reads a page of the project's history older than a revision", () =>
  Effect.gen(function* () {
    const { captured, provide } = captureRequest(() => json([entry("b", ["a"]), entry("a", [])]))
    const history = yield* provide(getProjectHistory(PROJECT, { before: "c", limit: 2 }))
    expect(captured.request?.url).toContain(`/api/projects/${PROJECT}/history`)
    expect(UrlParams.toString(captured.request!.urlParams)).toBe("before=c&limit=2")
    expect(history.map((one) => one.revision.id)).toEqual(["b", "a"])
  }),
)

it.effect("leaves `before` off for the newest page", () =>
  Effect.gen(function* () {
    const { captured, provide } = captureRequest(() => json([]))
    yield* provide(getProjectHistory(PROJECT, { limit: 50 }))
    expect(UrlParams.toString(captured.request!.urlParams)).toBe("limit=50")
  }),
)

it.effect("a page from a revision the daemon does not know carries the 404 reason", () =>
  Effect.gen(function* () {
    const { provide } = captureRequest(() => json({ message: "no such revision: c" }, 404))
    const result = yield* Effect.result(provide(getProjectHistory(PROJECT, { before: "c", limit: 2 })))
    const error = Result.isFailure(result) ? result.failure : undefined
    expect(error).toBeInstanceOf(TaskRejected)
    expect((error as TaskRejected).message).toContain("no such revision")
  }),
)

it.effect("reads a task as a revision left it", () =>
  Effect.gen(function* () {
    const { captured, provide } = captureRequest(() =>
      json({
        id: "OPP-1",
        revision: "abc",
        task: {
          title: "First",
          metadata: {
            status: "todo",
            created: "2026-01-01T00:00:00Z",
            parent: null,
            rank: null,
            dependencies: [],
            tags: [],
          },
          description: "The old body",
          raw: "---\nstatus: todo\n---\n# First\n\nThe old body\n",
        },
      }),
    )
    const at = yield* provide(getTaskRevision(PROJECT, "OPP-1", "abc"))
    expect(captured.request?.url).toContain(`/api/projects/${PROJECT}/tasks/OPP-1/revisions/abc`)
    expect(at.task?.description).toBe("The old body")
    expect(at.task?.comments).toBeUndefined()
  }),
)

it.effect("a sync posts to the project's route and decodes what moved", () =>
  Effect.gen(function* () {
    const { captured, provide } = captureRequest(() =>
      json({
        received: 2,
        sent: 1,
        merged: true,
        status: { remote: "origin", last_success: "2026-01-02T00:00:00Z", ahead: 0, behind: 0, syncing: false },
      }),
    )
    const result = yield* provide(runSync(PROJECT))
    expect(captured.request?.method).toBe("POST")
    expect(captured.request?.url).toContain(`/api/projects/${PROJECT}/sync`)
    expect(result.received).toBe(2)
    expect(result.status.ahead).toBe(0)
  }),
)

// A remote that cannot be reached is a 502 the route documents, and its reason is what a person acts
// on.
it.effect("a sync the remote refuses carries its reason", () =>
  Effect.gen(function* () {
    const { provide } = captureRequest(() => json({ message: "could not reach origin" }, 502))
    const result = yield* Effect.result(provide(runSync(PROJECT)))
    const error = Result.isFailure(result) ? result.failure : undefined
    expect((error as TaskRejected).status).toBe(502)
    expect((error as TaskRejected).message).toContain("could not reach origin")
  }),
)

it.effect("a text write puts the text and the text it started from, and decodes the task", () =>
  Effect.gen(function* () {
    const { captured, provide } = captureRequest(() =>
      json({
        project: "openplan",
        id: "OPP-1",
        title: "First",
        metadata: {
          status: "todo",
          created: "2026-01-01T00:00:00Z",
          parent: null,
          rank: null,
          dependencies: [],
          tags: [],
        },
        description: "Use OAuth only.\n",
        conflicts: 0,
        problems: [],
        updated: "2026-01-02T00:00:00Z",
      }),
    )
    const base = { title: "First", description: "" }
    const text = { title: "First", description: "Use OAuth only.\n" }
    const detail = yield* provide(writeTaskText(PROJECT, "OPP-1", base, text))
    expect(captured.request?.method).toBe("PUT")
    expect(captured.request?.url).toContain(`/api/projects/${PROJECT}/tasks/OPP-1/text`)
    expect(requestBody(captured.request!)).toEqual({ base, text })
    expect(detail.description).toBe("Use OAuth only.\n")
  }),
)

it.effect("a text write the daemon refuses carries its reason", () =>
  Effect.gen(function* () {
    const { provide } = captureRequest(() => json({ message: "a write cannot add a conflict or edit inside one" }, 400))
    const text = { title: "First", description: "x" }
    const result = yield* Effect.result(provide(writeTaskText(PROJECT, "OPP-1", text, text)))
    const error = Result.isFailure(result) ? result.failure : undefined
    expect(error).toBeInstanceOf(TaskRejected)
    expect((error as TaskRejected).status).toBe(400)
    expect((error as TaskRejected).message).toContain("cannot add a conflict")
  }),
)
