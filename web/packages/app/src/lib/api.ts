import { Context, Data, Effect } from "effect"
import type { Schema } from "effect"
import { HttpClient, type HttpClientError, HttpClientRequest, HttpClientResponse } from "effect/unstable/http"

import * as Api from "@open-planner/api-client"

export class TaskNotFound extends Data.TaggedError("TaskNotFound")<{
  readonly id: string
}> {}

// A write the server refused, carrying its reason ("cannot reparent … under its own descendant …").
// A bare status code would drop exactly the detail a user needs to understand why nothing happened.
export class TaskRejected extends Data.TaggedError("TaskRejected")<{
  readonly status: number
  readonly message: string
}> {}

export type ApiError = TaskNotFound | TaskRejected | HttpClientError.HttpClientError | Schema.SchemaError

// "" in the browser (same-origin, relative). Tests supply an absolute base because node's
// HttpClient cannot resolve a relative URL without a document origin.
export const ApiBaseUrl = Context.Reference<string>("app/ApiBaseUrl", {
  defaultValue: () => "",
})

const reason = (body: unknown, status: number): string =>
  typeof body === "object" && body !== null && typeof (body as Api.ApiErrorBody).message === "string"
    ? (body as Api.ApiErrorBody).message
    : `request failed with status ${status}`

// The daemon answers every refusal with an `ApiErrorBody`, and the generated client decodes exactly
// that to raise its typed errors. A failure body from anywhere else — a proxy's HTML page, an empty
// body from a dead connection — would decode-fail instead, losing the status along with it, so
// rewrite it into the documented shape and keep the status the thing that decides what happened.
const asReason = HttpClient.transformResponse(
  Effect.flatMap((response: HttpClientResponse.HttpClientResponse) =>
    response.status < 400
      ? Effect.succeed(response)
      : Effect.map(
          Effect.orElseSucceed(response.json, () => undefined),
          (body) =>
            HttpClientResponse.fromWeb(
              response.request,
              new Response(JSON.stringify({ message: reason(body, response.status) }), {
                status: response.status,
                headers: { "content-type": "application/json" },
              }),
            ),
        ),
  ),
)

const tasks: Effect.Effect<Api.TasksClient, never, HttpClient.HttpClient> = Effect.gen(function* () {
  const http = yield* HttpClient.HttpClient
  const base = yield* ApiBaseUrl
  return Api.make(http.pipe(HttpClient.mapRequest(HttpClientRequest.prependUrl(base)), asReason))
})

const refusal = (error: { readonly response: { readonly status: number }; readonly cause: Api.ApiErrorBody }) =>
  Effect.fail(new TaskRejected({ status: error.response.status, message: error.cause.message }))

// A status no route documents (a proxy's 502, say): the generated client folds it into a
// transport-shaped error, leaving the status as the only thing worth reporting.
const unexpected = (error: HttpClientError.HttpClientError) =>
  Effect.fail(
    error.reason._tag === "StatusCodeError"
      ? new TaskRejected({
          status: error.reason.response.status,
          message: `request failed with status ${error.reason.response.status}`,
        })
      : error,
  )

export const listProjects: Effect.Effect<
  ReadonlyArray<Api.ProjectView>,
  ApiError,
  HttpClient.HttpClient
> = Effect.flatMap(tasks, (client) => client.listProjects(undefined)).pipe(
  Effect.catchTags({ HttpClientError: unexpected }),
)

export const listTasks = (
  project: string,
): Effect.Effect<ReadonlyArray<Api.TaskListItem>, ApiError, HttpClient.HttpClient> =>
  Effect.flatMap(tasks, (client) => client.listTasks(project, undefined)).pipe(
    Effect.catchTags({
      ListTasks400: refusal,
      ListTasks404: refusal,
      ListTasks500: refusal,
      ListTasks503: refusal,
      HttpClientError: unexpected,
    }),
  )

// The tag registry a tag name resolves against. `branch` names the worktree that holds it — the same
// one a tags write is validated against — and omitting it reads the served worktree's.
export const listTags = (
  project: string,
  branch?: string,
): Effect.Effect<ReadonlyArray<Api.TagView>, ApiError, HttpClient.HttpClient> =>
  Effect.flatMap(tasks, (client) => client.listTags(project, { params: { branch } })).pipe(
    Effect.catchTags({
      ListTags400: refusal,
      ListTags404: refusal,
      ListTags409: refusal,
      ListTags422: refusal,
      ListTags500: refusal,
      ListTags503: refusal,
      HttpClientError: unexpected,
    }),
  )

// A tag is registered in one worktree's `.plan/tags`, so `branch` names the worktree the write lands
// in — the same one the tasks that reference it are written to.
export const createTag = (
  project: string,
  input: Api.CreateTag,
  branch?: string,
): Effect.Effect<Api.TagView, ApiError, HttpClient.HttpClient> =>
  Effect.flatMap(tasks, (client) => client.createTag(project, { payload: input, params: { branch } })).pipe(
    Effect.catchTags({
      CreateTag400: refusal,
      CreateTag404: refusal,
      CreateTag409: refusal,
      CreateTag422: refusal,
      CreateTag500: refusal,
      CreateTag503: refusal,
      HttpClientError: unexpected,
    }),
  )

// `name` renames the tag, which rewrites the `tags:` of every task on the branch that holds the old
// name; `color` and `description` change the tag file alone.
export const patchTag = (
  project: string,
  name: string,
  patch: Api.TagPatch,
  branch?: string,
): Effect.Effect<Api.TagView, ApiError, HttpClient.HttpClient> =>
  Effect.flatMap(tasks, (client) =>
    client.patchTag(project, encodeURIComponent(name), { payload: patch, params: { branch } }),
  ).pipe(
    Effect.catchTags({
      PatchTag400: refusal,
      PatchTag404: refusal,
      PatchTag409: refusal,
      PatchTag422: refusal,
      PatchTag500: refusal,
      PatchTag503: refusal,
      HttpClientError: unexpected,
    }),
  )

// Without `force` the daemon refuses while tasks on this branch reference the tag, and says how many
// do. With it the tag goes and those references are left dangling, so it is never sent unasked.
export const deleteTag = (
  project: string,
  name: string,
  force: boolean,
  branch?: string,
): Effect.Effect<void, ApiError, HttpClient.HttpClient> =>
  Effect.flatMap(tasks, (client) =>
    client.deleteTag(project, encodeURIComponent(name), { params: { force, branch } }),
  ).pipe(
    Effect.catchTags({
      DeleteTag400: refusal,
      DeleteTag404: refusal,
      DeleteTag409: refusal,
      DeleteTag500: refusal,
      DeleteTag503: refusal,
      HttpClientError: unexpected,
    }),
  )

// The board of one project, which its own route answers for.
export const getBoard = (project: string): Effect.Effect<Api.Board, ApiError, HttpClient.HttpClient> =>
  Effect.flatMap(tasks, (client) => client.getBoard(project, undefined)).pipe(
    Effect.catchTags({
      GetBoard400: refusal,
      GetBoard404: refusal,
      GetBoard500: refusal,
      GetBoard503: refusal,
      HttpClientError: unexpected,
    }),
  )

// Every servable project's board in one read, which the UI opens on. Each row carries its project,
// so a key that exists in two stores stays two rows.
export const getMergedBoard: Effect.Effect<Api.Board, ApiError, HttpClient.HttpClient> = Effect.flatMap(
  tasks,
  (client) => client.getMergedBoard(undefined),
).pipe(
  Effect.catchTags({
    GetMergedBoard500: refusal,
    HttpClientError: unexpected,
  }),
)

// Every servable project at once, matching the merged board the palette opens over. An empty query
// matches nothing, so the caller may send every keystroke.
export const searchTasks = (
  query: string,
): Effect.Effect<ReadonlyArray<Api.SearchHit>, ApiError, HttpClient.HttpClient> =>
  Effect.flatMap(tasks, (client) => client.searchAll({ params: { q: query } })).pipe(
    Effect.catchTags({
      SearchAll500: refusal,
      HttpClientError: unexpected,
    }),
  )

// Omitting `branch` returns the headline (current-worktree) version; passing one returns that
// branch's version. Either way the response carries every branch the task lives on.
export const getTask = (
  project: string,
  id: string,
  branch?: string,
): Effect.Effect<Api.TaskDetail, ApiError, HttpClient.HttpClient> =>
  Effect.flatMap(tasks, (client) => client.getTask(project, encodeURIComponent(id), { params: { branch } })).pipe(
    Effect.catchTags({
      GetTask400: refusal,
      GetTask404: () => Effect.fail(new TaskNotFound({ id })),
      GetTask500: refusal,
      GetTask503: refusal,
      HttpClientError: unexpected,
    }),
  )

// `parent` is three-state to match the server: omit the key to leave it unchanged, JSON `null` to
// clear it (top level), or an id to set it. `rank`, `status`, and `dependencies` are set-only.
// `branch` writes that one branch's version. The page passes the branch it is showing, so an edit
// changes the version on screen; omitting it leaves the daemon to write wherever the task lives.
export const patchTask = (
  project: string,
  id: string,
  patch: Api.TaskPatch,
  branch?: string,
): Effect.Effect<Api.TaskDetail, ApiError, HttpClient.HttpClient> =>
  Effect.flatMap(tasks, (client) =>
    client.patchTask(project, encodeURIComponent(id), { payload: patch, params: { branch } }),
  ).pipe(
    Effect.catchTags({
      PatchTask400: refusal,
      PatchTask404: () => Effect.fail(new TaskNotFound({ id })),
      PatchTask409: refusal,
      PatchTask500: refusal,
      PatchTask503: refusal,
      HttpClientError: unexpected,
    }),
  )

// `branch` puts the new task on one branch's worktree. A subtask names the branch its parent lives
// on, so the pair stays together instead of parting on whatever the daemon's root has checked out.
export const createTask = (
  project: string,
  input: Api.CreateTask,
  branch?: string,
): Effect.Effect<string, ApiError, HttpClient.HttpClient> =>
  Effect.flatMap(tasks, (client) => client.createTask(project, { payload: input, params: { branch } })).pipe(
    Effect.map((created) => created.id),
    Effect.catchTags({
      CreateTask400: refusal,
      CreateTask404: refusal,
      CreateTask409: refusal,
      CreateTask500: refusal,
      CreateTask503: refusal,
      HttpClientError: unexpected,
    }),
  )
