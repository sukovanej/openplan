import { Context, Data, Effect } from "effect"
import type { Schema } from "effect"
import { HttpClient, type HttpClientError, HttpClientRequest, HttpClientResponse } from "effect/unstable/http"

import * as Api from "@openplan/api-client"

import type { FlowSelection } from "./flow-selection"

export class TaskNotFound extends Data.TaggedError("TaskNotFound")<{
  readonly id: string
}> {}

// A write the server refused, carrying its reason ("cannot reparent … under its own descendant …").
// A bare status code would drop exactly the detail a user needs to understand why nothing happened.
// `reason` names the refusals a caller acts on rather than reads: one status covers several, and
// only the field tells them apart.
export class TaskRejected extends Data.TaggedError("TaskRejected")<{
  readonly status: number
  readonly message: string
  readonly reason?: Api.Refusal
}> {}

// A flow the daemon cannot order, with the members of each cycle it found. The keys carry no
// project, so the view names them rather than linking them.
export class FlowCycles extends Data.TaggedError("FlowCycles")<{
  readonly message: string
  readonly cycles: ReadonlyArray<ReadonlyArray<string>>
}> {}

// A diagram source the daemon cannot read, with the place it stopped.
export class DiagramRefused extends Data.TaggedError("DiagramRefused")<{
  readonly message: string
  readonly position?: Api.SourcePosition
}> {}

export type ApiError = TaskNotFound | TaskRejected | HttpClientError.HttpClientError | Schema.SchemaError

// "" in the browser (same-origin, relative). Tests supply an absolute base because node's
// HttpClient cannot resolve a relative URL without a document origin.
export const ApiBaseUrl = Context.Reference<string>("app/ApiBaseUrl", {
  defaultValue: () => "",
})

// `from` names the document on the parent's side, where a new title moved a task to a new file.
export interface DiffTarget {
  readonly path: string
  readonly from?: string
}

export interface HistoryPage {
  readonly before?: string
  readonly limit: number
}

const asBody = (body: unknown, status: number): Api.ApiErrorBody =>
  typeof body === "object" && body !== null && typeof (body as Api.ApiErrorBody).message === "string"
    ? (body as Api.ApiErrorBody)
    : { message: `request failed with status ${status}` }

// The daemon answers every refusal with an `ApiErrorBody`, and the generated client decodes exactly
// that to raise its typed errors. A failure body from anywhere else — a proxy's HTML page, an empty
// body from a dead connection — would decode-fail instead, losing the status along with it, so
// rewrite it into the documented shape and keep the status the thing that decides what happened. A
// body that already carries a message passes through whole, because a route can add a field to it.
const asReason = HttpClient.transformResponse(
  Effect.flatMap((response: HttpClientResponse.HttpClientResponse) =>
    response.status < 400
      ? Effect.succeed(response)
      : Effect.map(
          Effect.orElseSucceed(response.json, () => undefined),
          (body) =>
            HttpClientResponse.fromWeb(
              response.request,
              new Response(JSON.stringify(asBody(body, response.status)), {
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
  Effect.fail(
    new TaskRejected({
      status: error.response.status,
      message: error.cause.message,
      reason: error.cause.reason,
    }),
  )

const parsed = (text: string | undefined): unknown => {
  try {
    return JSON.parse(text ?? "")
  } catch {
    return undefined
  }
}

// A status no route documents — the daemon's own 500, or a proxy's 502 — reaches here folded into a
// transport-shaped error. `asReason` already gave its body the documented shape, and the generated
// client keeps that body as the description, so the reason survives the fold.
const unexpected = (error: HttpClientError.HttpClientError) => {
  if (error.reason._tag !== "StatusCodeError") return Effect.fail(error)
  const status = error.reason.response.status
  const body = asBody(parsed(error.reason.description), status)
  return Effect.fail(new TaskRejected({ status, message: body.message, reason: body.reason }))
}

export const drawDiagram = (
  source: string,
): Effect.Effect<Api.Drawing, ApiError | DiagramRefused, HttpClient.HttpClient> =>
  Effect.flatMap(tasks, (client) => client.drawDiagram({ payload: { source } })).pipe(
    Effect.catchTags({
      DrawDiagram422: (error) =>
        Effect.fail(new DiagramRefused({ message: error.cause.message, position: error.cause.position })),
      HttpClientError: unexpected,
    }),
  )

export interface PageSize {
  readonly width: number
  readonly height: number
}

export const drawFlow = (
  selection: FlowSelection,
  page: PageSize,
): Effect.Effect<Api.Drawing, ApiError | FlowCycles, HttpClient.HttpClient> =>
  Effect.flatMap(tasks, (client) =>
    client.drawFlow({
      params: {
        project: selection.projects,
        // The daemon is what knows the status names, and it refuses one it cannot read with a 400
        // that says so; a client-side list of them would be a second place to keep them in step.
        status: selection.statuses as ReadonlyArray<Api.Status>,
        task: selection.tasks,
        tag: selection.tags,
        width: page.width,
        height: page.height,
      },
    }),
  ).pipe(
    Effect.catchTags({
      DrawFlow400: refusal,
      DrawFlow404: refusal,
      DrawFlow422: (error) =>
        Effect.fail(new FlowCycles({ message: error.cause.message, cycles: error.cause.cycles ?? [] })),
      DrawFlow503: refusal,
      HttpClientError: unexpected,
    }),
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
      ListTasks404: refusal,
      ListTasks503: refusal,
      HttpClientError: unexpected,
    }),
  )

export const listTags = (project: string): Effect.Effect<ReadonlyArray<Api.TagView>, ApiError, HttpClient.HttpClient> =>
  Effect.flatMap(tasks, (client) => client.listTags(project, undefined)).pipe(
    Effect.catchTags({
      ListTags404: refusal,
      ListTags422: refusal,
      ListTags503: refusal,
      HttpClientError: unexpected,
    }),
  )

export const createTag = (
  project: string,
  input: Api.CreateTag,
): Effect.Effect<Api.TagView, ApiError, HttpClient.HttpClient> =>
  Effect.flatMap(tasks, (client) => client.createTag(project, { payload: input })).pipe(
    Effect.catchTags({
      CreateTag400: refusal,
      CreateTag404: refusal,
      CreateTag409: refusal,
      CreateTag422: refusal,
      CreateTag503: refusal,
      HttpClientError: unexpected,
    }),
  )

// `name` renames the tag, which rewrites the `tags:` of every task that holds the old name; `color`
// and `description` change the tag file alone.
export const patchTag = (
  project: string,
  name: string,
  patch: Api.TagPatch,
): Effect.Effect<Api.TagView, ApiError, HttpClient.HttpClient> =>
  Effect.flatMap(tasks, (client) => client.patchTag(project, name, { payload: patch })).pipe(
    Effect.catchTags({
      PatchTag400: refusal,
      PatchTag404: refusal,
      PatchTag409: refusal,
      PatchTag422: refusal,
      PatchTag503: refusal,
      HttpClientError: unexpected,
    }),
  )

// Without `force` the daemon refuses while tasks reference the tag, and says how many do. With it the
// tag goes and those references are left dangling, so it is never sent unasked.
export const deleteTag = (
  project: string,
  name: string,
  force: boolean,
): Effect.Effect<void, ApiError, HttpClient.HttpClient> =>
  Effect.flatMap(tasks, (client) => client.deleteTag(project, name, { params: { force } })).pipe(
    Effect.catchTags({
      DeleteTag400: refusal,
      DeleteTag404: refusal,
      DeleteTag409: refusal,
      DeleteTag503: refusal,
      HttpClientError: unexpected,
    }),
  )

// The board of one project, which its own route answers for.
export const getBoard = (project: string): Effect.Effect<Api.Board, ApiError, HttpClient.HttpClient> =>
  Effect.flatMap(tasks, (client) => client.getBoard(project, undefined)).pipe(
    Effect.catchTags({
      GetBoard404: refusal,
      GetBoard503: refusal,
      HttpClientError: unexpected,
    }),
  )

// Every servable project's board in one read, which the UI opens on. Each row carries its project,
// so a key that exists in two stores stays two rows.
export const getMergedBoard: Effect.Effect<Api.Board, ApiError, HttpClient.HttpClient> = Effect.flatMap(
  tasks,
  (client) => client.getMergedBoard(undefined),
).pipe(Effect.catchTags({ HttpClientError: unexpected }))

// Every servable project at once, matching the merged board the palette opens over. An empty query
// matches nothing, so the caller may send every keystroke.
export const searchTasks = (
  query: string,
): Effect.Effect<ReadonlyArray<Api.SearchHit>, ApiError, HttpClient.HttpClient> =>
  Effect.flatMap(tasks, (client) => client.searchAll({ params: { q: query } })).pipe(
    Effect.catchTags({ HttpClientError: unexpected }),
  )

export const getTask = (project: string, id: string): Effect.Effect<Api.TaskDetail, ApiError, HttpClient.HttpClient> =>
  Effect.flatMap(tasks, (client) => client.getTask(project, id, undefined)).pipe(
    Effect.catchTags({
      GetTask400: refusal,
      GetTask404: () => Effect.fail(new TaskNotFound({ id })),
      GetTask503: refusal,
      HttpClientError: unexpected,
    }),
  )

// `parent` is three-state to match the server: omit the key to leave it unchanged, JSON `null` to
// clear it (top level), or an id to set it. `rank`, `status`, and `dependencies` are set-only.
export const patchTask = (
  project: string,
  id: string,
  patch: Api.TaskPatch,
): Effect.Effect<Api.TaskDetail, ApiError, HttpClient.HttpClient> =>
  Effect.flatMap(tasks, (client) => client.patchTask(project, id, { payload: patch })).pipe(
    Effect.catchTags({
      PatchTask400: refusal,
      PatchTask404: () => Effect.fail(new TaskNotFound({ id })),
      PatchTask409: refusal,
      PatchTask503: refusal,
      HttpClientError: unexpected,
    }),
  )

// `base` is the text the edit started from. The daemon merges the edit with any change another writer
// made since, and answers with the task it wrote.
export const writeTaskText = (
  project: string,
  id: string,
  base: Api.TaskText,
  text: Api.TaskText,
): Effect.Effect<Api.TaskDetail, ApiError, HttpClient.HttpClient> =>
  Effect.flatMap(tasks, (client) => client.writeText(project, id, { payload: { base, text } })).pipe(
    Effect.catchTags({
      WriteText400: refusal,
      WriteText404: () => Effect.fail(new TaskNotFound({ id })),
      WriteText409: refusal,
      WriteText503: refusal,
      HttpClientError: unexpected,
    }),
  )

export const createTask = (
  project: string,
  input: Api.CreateTask,
): Effect.Effect<string, ApiError, HttpClient.HttpClient> =>
  Effect.flatMap(tasks, (client) => client.createTask(project, { payload: input })).pipe(
    Effect.map((created) => created.id),
    Effect.catchTags({
      CreateTask400: refusal,
      CreateTask404: refusal,
      CreateTask409: refusal,
      CreateTask503: refusal,
      HttpClientError: unexpected,
    }),
  )

export const getProjectHistory = (
  project: string,
  page: HistoryPage,
): Effect.Effect<ReadonlyArray<Api.HistoryEntry>, ApiError, HttpClient.HttpClient> =>
  Effect.flatMap(tasks, (client) => client.projectHistory(project, { params: page })).pipe(
    Effect.catchTags({
      ProjectHistory404: refusal,
      ProjectHistory503: refusal,
      HttpClientError: unexpected,
    }),
  )

export const getTaskHistory = (
  project: string,
  id: string,
  page: HistoryPage,
): Effect.Effect<ReadonlyArray<Api.HistoryEntry>, ApiError, HttpClient.HttpClient> =>
  Effect.flatMap(tasks, (client) => client.taskHistory(project, id, { params: page })).pipe(
    Effect.catchTags({
      TaskHistory400: refusal,
      TaskHistory404: refusal,
      TaskHistory503: refusal,
      HttpClientError: unexpected,
    }),
  )

export const getRevisionDiff = (
  project: string,
  revision: string,
  target: DiffTarget,
): Effect.Effect<Api.DocumentDiff, ApiError, HttpClient.HttpClient> =>
  Effect.flatMap(tasks, (client) => client.revisionDiff(project, revision, { params: target })).pipe(
    Effect.catchTags({
      RevisionDiff404: refusal,
      RevisionDiff503: refusal,
      HttpClientError: unexpected,
    }),
  )

export const getTaskRevision = (
  project: string,
  id: string,
  revision: string,
): Effect.Effect<Api.TaskAtRevision, ApiError, HttpClient.HttpClient> =>
  Effect.flatMap(tasks, (client) => client.taskRevision(project, id, revision, undefined)).pipe(
    Effect.catchTags({
      TaskRevision400: refusal,
      TaskRevision404: refusal,
      TaskRevision503: refusal,
      HttpClientError: unexpected,
    }),
  )

export const getSync = (project: string): Effect.Effect<Api.SyncView, ApiError, HttpClient.HttpClient> =>
  Effect.flatMap(tasks, (client) => client.getSync(project, undefined)).pipe(
    Effect.catchTags({
      GetSync404: refusal,
      GetSync503: refusal,
      HttpClientError: unexpected,
    }),
  )

export const runSync = (project: string): Effect.Effect<Api.SyncResult, ApiError, HttpClient.HttpClient> =>
  Effect.flatMap(tasks, (client) => client.runSync(project, undefined)).pipe(
    Effect.catchTags({
      RunSync404: refusal,
      RunSync502: refusal,
      RunSync503: refusal,
      HttpClientError: unexpected,
    }),
  )
