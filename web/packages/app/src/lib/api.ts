import { Context, Data, Effect, Schema } from "effect"
import { HttpClient, HttpClientError, HttpClientResponse } from "effect/unstable/http"

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
  deps: Schema.optionalKey(Schema.Array(Schema.String)),
  body: Schema.String,
  headline: Schema.String,
  branches: Schema.Array(BranchState),
})
export type TaskDetail = typeof TaskDetail.Type

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
