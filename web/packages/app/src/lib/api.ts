import { Context, Data, Effect, Schema } from "effect"
import { HttpClient, HttpClientError, HttpClientResponse } from "effect/unstable/http"

export const Status = Schema.Literals([
  "backlog",
  "todo",
  "in_progress",
  "done",
  "cancelled",
])
export type Status = typeof Status.Type

export const TaskSummary = Schema.Struct({
  id: Schema.String,
  title: Schema.String,
  status: Status,
  parent: Schema.optionalKey(Schema.String),
})
export type TaskSummary = typeof TaskSummary.Type

export const TaskView = Schema.Struct({
  id: Schema.String,
  title: Schema.String,
  status: Status,
  parent: Schema.optionalKey(Schema.String),
  deps: Schema.optionalKey(Schema.Array(Schema.String)),
  body: Schema.String,
})
export type TaskView = typeof TaskView.Type

const TaskSummaries = Schema.Array(TaskSummary)

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
  ReadonlyArray<TaskSummary>,
  HttpClientError.HttpClientError | Schema.SchemaError,
  HttpClient.HttpClient
> = Effect.gen(function*() {
  const client = yield* HttpClient.HttpClient
  const base = yield* ApiBaseUrl
  const response = yield* client.get(`${base}/api/tasks`)
  return yield* HttpClientResponse.schemaBodyJson(TaskSummaries)(response)
}).pipe(Effect.scoped)

export const getTask = (
  id: string,
): Effect.Effect<TaskView, TaskError, HttpClient.HttpClient> =>
  Effect.gen(function*() {
    const client = yield* HttpClient.HttpClient
    const base = yield* ApiBaseUrl
    const response = yield* client.get(`${base}/api/tasks/${encodeURIComponent(id)}`)
    if (response.status === 404) {
      return yield* Effect.fail(new TaskNotFound({ id }))
    }
    return yield* HttpClientResponse.schemaBodyJson(TaskView)(response)
  }).pipe(Effect.scoped)
