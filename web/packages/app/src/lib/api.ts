import { Context, Data, Effect, Schema } from "effect"
import { HttpClient, HttpClientError, HttpClientRequest, HttpClientResponse } from "effect/unstable/http"

export const Status = Schema.Literals([
  "backlog",
  "todo",
  "in_progress",
  "in_review",
  "done",
  "cancelled",
])
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

// A task's view on one branch plus every branch it lives on, so the detail page can render a
// branch switcher even when loaded cold (no list in memory).
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
})
export type TaskDetail = typeof TaskDetail.Type

// The subtree the daemon builds by grouping the flat task set on `parent`; siblings arrive in
// `rank` order.
export interface TaskTree {
  readonly id: string
  readonly title: string
  readonly status: Status
  readonly parent?: string
  readonly rank?: string
  readonly children: ReadonlyArray<TaskTree>
}
export const TaskTree: Schema.Codec<TaskTree> = Schema.Struct({
  id: Schema.String,
  title: Schema.String,
  status: Status,
  parent: Schema.optionalKey(Schema.String),
  rank: Schema.optionalKey(Schema.String),
  children: Schema.Array(Schema.suspend((): Schema.Codec<TaskTree> => TaskTree)),
})

const TaskListItems = Schema.Array(TaskListItem)

export class TaskNotFound extends Data.TaggedError("TaskNotFound")<{
  readonly id: string
}> {}

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
> = Effect.gen(function*() {
  const client = yield* HttpClient.HttpClient
  const base = yield* ApiBaseUrl
  const response = yield* client.get(`${base}/api/tasks`)
  return yield* HttpClientResponse.schemaBodyJson(TaskListItems)(response)
}).pipe(Effect.scoped)

// Omitting `branch` returns the headline (current-worktree) version; passing one returns that
// branch's version. Either way the response carries every branch the task lives on.
export const getTask = (
  id: string,
  branch?: string,
): Effect.Effect<TaskDetail, TaskError, HttpClient.HttpClient> =>
  Effect.gen(function*() {
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

// The subtree rooted at `id`, bounded by `depth` (direct children at `1`, unbounded when omitted).
export const getTree = (
  id: string,
  depth?: number,
): Effect.Effect<TaskTree, TaskError, HttpClient.HttpClient> =>
  Effect.gen(function*() {
    const client = yield* HttpClient.HttpClient
    const base = yield* ApiBaseUrl
    const query = depth === undefined ? "" : `?depth=${depth}`
    const response = yield* client.get(`${base}/api/tasks/${encodeURIComponent(id)}/tree${query}`)
    if (response.status === 404) {
      return yield* Effect.fail(new TaskNotFound({ id }))
    }
    const ok = yield* HttpClientResponse.filterStatusOk(response)
    return yield* HttpClientResponse.schemaBodyJson(TaskTree)(ok)
  }).pipe(Effect.scoped)

// `parent` is three-state to match the server: omit the key to leave it unchanged, `null` to clear
// it (top level), or an id to set it. `rank` and `status` are set-only.
export interface TaskPatch {
  readonly status?: Status
  readonly parent?: string | null
  readonly rank?: string
}

export const patchTask = (id: string, patch: TaskPatch) =>
  Effect.gen(function*() {
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
    const ok = yield* HttpClientResponse.filterStatusOk(response)
    return yield* HttpClientResponse.schemaBodyJson(TaskDetail)(ok)
  }).pipe(Effect.scoped)

export interface CreateTask {
  readonly title: string
  readonly parent?: string
  readonly status?: Status
}

const CreatedTask = Schema.Struct({ id: Schema.String })

export const createTask = (input: CreateTask) =>
  Effect.gen(function*() {
    const client = yield* HttpClient.HttpClient
    const base = yield* ApiBaseUrl
    const request = yield* HttpClientRequest.bodyJson(
      HttpClientRequest.post(`${base}/api/tasks`),
      input,
    )
    const response = yield* client.execute(request)
    const ok = yield* HttpClientResponse.filterStatusOk(response)
    const created = yield* HttpClientResponse.schemaBodyJson(CreatedTask)(ok)
    return created.id
  }).pipe(Effect.scoped)
