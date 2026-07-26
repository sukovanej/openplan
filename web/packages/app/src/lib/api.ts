import { Context, Data, Effect, Schema } from "effect"
import { HttpClient, HttpClientError, HttpClientRequest, HttpClientResponse } from "effect/unstable/http"

export const Status = Schema.Literals(["backlog", "todo", "in_progress", "in_review", "done", "cancelled"])
export type Status = typeof Status.Type

export const ChangeKind = Schema.Literals(["base", "added", "modified", "deleted"])
export type ChangeKind = typeof ChangeKind.Type

export const BranchState = Schema.Struct({
  branch: Schema.String,
  status: Status,
  blob_oid: Schema.String,
  dirty: Schema.Boolean,
  kind: ChangeKind,
})
export type BranchState = typeof BranchState.Type

// One logical task aggregated across branches: `status`/`title` come from the `headline` branch (the
// most recently changed one), and `branches` carries every branch it lives on for badges/divergence.
export const TaskListItem = Schema.Struct({
  id: Schema.String,
  title: Schema.String,
  status: Status,
  parent: Schema.optionalKey(Schema.String),
  rank: Schema.optionalKey(Schema.String),
  headline: Schema.String,
  branches: Schema.Array(BranchState),
})
export type TaskListItem = typeof TaskListItem.Type

// A direct child of a task, in sibling (`rank`) order — the subtasks list renders straight from this.
export const TaskChild = Schema.Struct({
  id: Schema.String,
  title: Schema.String,
  status: Status,
  rank: Schema.optionalKey(Schema.String),
})
export type TaskChild = typeof TaskChild.Type

// A task referenced by `[[id]]` in the body, resolved to its current title/status so a chip renders
// without a full-list lookup.
export const TaskRef = Schema.Struct({
  id: Schema.String,
  title: Schema.String,
  status: Status,
})
export type TaskRef = typeof TaskRef.Type

// A task's view on one branch plus every branch it lives on, so the detail page can render a branch
// switcher even when loaded cold (no list in memory). `parent_title`, `children`, and `refs` carry
// the immediate hierarchy so the page renders from this one read rather than the whole task set.
export const TaskDetail = Schema.Struct({
  id: Schema.String,
  title: Schema.String,
  status: Status,
  parent: Schema.optionalKey(Schema.String),
  rank: Schema.optionalKey(Schema.String),
  deps: Schema.optionalKey(Schema.Array(Schema.String)),
  body: Schema.String,
  headline: Schema.String,
  branches: Schema.Array(BranchState),
  parent_title: Schema.optionalKey(Schema.String),
  children: Schema.optionalKey(Schema.Array(TaskChild)),
  refs: Schema.optionalKey(Schema.Array(TaskRef)),
})
export type TaskDetail = typeof TaskDetail.Type

// One rendered line of the board: the task, its `depth` within the status group (a group-local root
// is 0), whether a nested row sits beneath it, and — for a group-local root whose real parent lives
// in another status group — that parent's title for a "Subtask of <parent>" hint.
export const BoardRow = Schema.Struct({
  task: TaskListItem,
  depth: Schema.Number,
  has_children: Schema.Boolean,
  parent_title: Schema.optionalKey(Schema.String),
})
export type BoardRow = typeof BoardRow.Type

export const BoardGroup = Schema.Struct({
  status: Status,
  rows: Schema.Array(BoardRow),
})
export type BoardGroup = typeof BoardGroup.Type

// The whole list view in one read: status groups in render order, each already flattened into
// ordered rows. The client renders it verbatim — the server owns grouping, ordering, and hierarchy.
export const Board = Schema.Struct({
  groups: Schema.Array(BoardGroup),
})
export type Board = typeof Board.Type

const TaskListItems = Schema.Array(TaskListItem)

export class TaskNotFound extends Data.TaggedError("TaskNotFound")<{
  readonly id: string
}> {}

// A write the server refused, carrying its reason ("cannot reparent … under its own descendant …").
// `filterStatusOk` alone would reduce that to a bare status code, which is exactly the detail a user
// needs to understand why nothing happened.
export class TaskRejected extends Data.TaggedError("TaskRejected")<{
  readonly status: number
  readonly message: string
}> {}

const ApiErrorBody = Schema.Struct({ message: Schema.String })

const rejected = (response: HttpClientResponse.HttpClientResponse): Effect.Effect<never, TaskRejected> =>
  HttpClientResponse.schemaBodyJson(ApiErrorBody)(response).pipe(
    Effect.catch(() => Effect.succeed({ message: `request failed with status ${response.status}` })),
    Effect.flatMap((body) => Effect.fail(new TaskRejected({ status: response.status, message: body.message }))),
  )

export type TaskError = TaskNotFound | HttpClientError.HttpClientError | Schema.SchemaError

// "" in the browser (same-origin, relative). Tests supply an absolute base because node's
// HttpClient cannot resolve a relative URL without a document origin.
export const ApiBaseUrl = Context.Reference<string>("app/ApiBaseUrl", {
  defaultValue: () => "",
})

export const listTasks: Effect.Effect<
  ReadonlyArray<TaskListItem>,
  HttpClientError.HttpClientError | Schema.SchemaError,
  HttpClient.HttpClient
> = Effect.gen(function* () {
  const client = yield* HttpClient.HttpClient
  const base = yield* ApiBaseUrl
  const response = yield* client.get(`${base}/api/tasks`)
  return yield* HttpClientResponse.schemaBodyJson(TaskListItems)(response)
}).pipe(Effect.scoped)

export const getBoard: Effect.Effect<
  Board,
  HttpClientError.HttpClientError | Schema.SchemaError,
  HttpClient.HttpClient
> = Effect.gen(function* () {
  const client = yield* HttpClient.HttpClient
  const base = yield* ApiBaseUrl
  const response = yield* client.get(`${base}/api/board`)
  return yield* HttpClientResponse.schemaBodyJson(Board)(response)
}).pipe(Effect.scoped)

// Omitting `branch` returns the headline (current-worktree) version; passing one returns that
// branch's version. Either way the response carries every branch the task lives on.
export const getTask = (id: string, branch?: string): Effect.Effect<TaskDetail, TaskError, HttpClient.HttpClient> =>
  Effect.gen(function* () {
    const client = yield* HttpClient.HttpClient
    const base = yield* ApiBaseUrl
    const query = branch === undefined ? "" : `?branch=${encodeURIComponent(branch)}`
    const response = yield* client.get(`${base}/api/tasks/${encodeURIComponent(id)}${query}`)
    if (response.status === 404) {
      return yield* Effect.fail(new TaskNotFound({ id }))
    }
    const ok = yield* HttpClientResponse.filterStatusOk(response)
    return yield* HttpClientResponse.schemaBodyJson(TaskDetail)(ok)
  }).pipe(Effect.scoped)

// `parent` is three-state to match the server: omit the key to leave it unchanged, `null` to clear
// it (top level), or an id to set it. `rank` and `status` are set-only.
export interface TaskPatch {
  readonly status?: Status
  readonly parent?: string | null
  readonly rank?: string
}

export const patchTask = (id: string, patch: TaskPatch) =>
  Effect.gen(function* () {
    const client = yield* HttpClient.HttpClient
    const base = yield* ApiBaseUrl
    const request = yield* HttpClientRequest.bodyJson(
      HttpClientRequest.patch(`${base}/api/tasks/${encodeURIComponent(id)}`),
      patch,
    )
    const response = yield* client.execute(request)
    if (response.status === 404) {
      return yield* Effect.fail(new TaskNotFound({ id }))
    }
    if (response.status >= 400) {
      return yield* rejected(response)
    }
    return yield* HttpClientResponse.schemaBodyJson(TaskDetail)(response)
  }).pipe(Effect.scoped)

export interface CreateTask {
  readonly title: string
  readonly parent?: string
  readonly status?: Status
}

const CreatedTask = Schema.Struct({ id: Schema.String })

export const createTask = (input: CreateTask) =>
  Effect.gen(function* () {
    const client = yield* HttpClient.HttpClient
    const base = yield* ApiBaseUrl
    const request = yield* HttpClientRequest.bodyJson(HttpClientRequest.post(`${base}/api/tasks`), input)
    const response = yield* client.execute(request)
    if (response.status >= 400) {
      return yield* rejected(response)
    }
    const created = yield* HttpClientResponse.schemaBodyJson(CreatedTask)(response)
    return created.id
  }).pipe(Effect.scoped)
