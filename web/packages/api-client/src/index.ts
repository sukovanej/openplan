import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import type { SchemaError } from "effect/Schema"
import * as Schema from "effect/Schema"
import type * as HttpClient from "effect/unstable/http/HttpClient"
import * as HttpClientError from "effect/unstable/http/HttpClientError"
import * as HttpClientRequest from "effect/unstable/http/HttpClientRequest"
import * as HttpClientResponse from "effect/unstable/http/HttpClientResponse"
// non-recursive definitions
export type ApiErrorBody = { readonly "message": string }
export const ApiErrorBody = Schema.Struct({ "message": Schema.String })
export type ChangeKind = "base" | "added" | "modified" | "deleted"
export const ChangeKind = Schema.Literals(["base", "added", "modified", "deleted"])
export type CreatedTask = { readonly "id": string }
export const CreatedTask = Schema.Struct({ "id": Schema.String })
export type DaemonInfo = {
  readonly "pid": number
  readonly "port": number
  readonly "started_at": number
  readonly "version": string
}
export const DaemonInfo = Schema.Struct({
  "pid": Schema.Number.annotate({ "format": "int32" }).check(Schema.isInt()).check(Schema.isGreaterThanOrEqualTo(0)),
  "port": Schema.Number.annotate({ "format": "int32" }).check(Schema.isInt()).check(Schema.isGreaterThanOrEqualTo(0)),
  "started_at": Schema.Number.annotate({ "format": "int64" }).check(Schema.isInt()).check(
    Schema.isGreaterThanOrEqualTo(0),
  ),
  "version": Schema.String,
})
export type Status = "backlog" | "todo" | "in_progress" | "in_review" | "done" | "cancelled"
export const Status = Schema.Literals(["backlog", "todo", "in_progress", "in_review", "done", "cancelled"])
export type BranchState = {
  readonly "blob_oid": string
  readonly "branch": string
  readonly "dirty": boolean
  readonly "kind": ChangeKind
  readonly "status": Status
}
export const BranchState = Schema.Struct({
  "blob_oid": Schema.String,
  "branch": Schema.String,
  "dirty": Schema.Boolean,
  "kind": ChangeKind,
  "status": Status,
})
export type CreateTask = {
  readonly "body"?: string | null
  readonly "deps"?: ReadonlyArray<string>
  readonly "parent"?: string | null
  readonly "status"?: null | Status
  readonly "title": string
}
export const CreateTask = Schema.Struct({
  "body": Schema.optionalKey(Schema.Union([Schema.String, Schema.Null])),
  "deps": Schema.optionalKey(Schema.Array(Schema.String)),
  "parent": Schema.optionalKey(Schema.Union([Schema.String, Schema.Null])),
  "status": Schema.optionalKey(Schema.Union([Schema.Null, Status], { mode: "oneOf" })),
  "title": Schema.String,
})
export type TaskPatch = {
  readonly "deps"?: ReadonlyArray<string>
  readonly "parent"?: string | null
  readonly "status"?: null | Status
}
export const TaskPatch = Schema.Struct({
  "deps": Schema.optionalKey(Schema.Union([Schema.Array(Schema.String)])),
  "parent": Schema.optionalKey(Schema.Union([Schema.String, Schema.Null])),
  "status": Schema.optionalKey(Schema.Union([Schema.Null, Status], { mode: "oneOf" })),
})
export type TaskDetail = {
  readonly "body": string
  readonly "deps"?: ReadonlyArray<string>
  readonly "id": string
  readonly "parent"?: string | null
  readonly "status": Status
  readonly "title": string
  readonly "branches": ReadonlyArray<BranchState>
  readonly "headline": string
}
export const TaskDetail = Schema.Struct({
  "body": Schema.String,
  "deps": Schema.optionalKey(Schema.Array(Schema.String)),
  "id": Schema.String,
  "parent": Schema.optionalKey(Schema.Union([Schema.String, Schema.Null])),
  "status": Status,
  "title": Schema.String,
  "branches": Schema.Array(BranchState),
  "headline": Schema.String,
})
export type TaskListItem = {
  readonly "branches": ReadonlyArray<BranchState>
  readonly "headline": string
  readonly "id": string
  readonly "parent"?: string | null
  readonly "status": Status
  readonly "title": string
}
export const TaskListItem = Schema.Struct({
  "branches": Schema.Array(BranchState),
  "headline": Schema.String,
  "id": Schema.String,
  "parent": Schema.optionalKey(Schema.Union([Schema.String, Schema.Null])),
  "status": Status,
  "title": Schema.String,
})
// schemas
export type ListTasks200 = ReadonlyArray<TaskListItem>
export const ListTasks200 = Schema.Array(TaskListItem)
export type CreateTaskRequestJson = CreateTask
export const CreateTaskRequestJson = CreateTask
export type CreateTask201 = CreatedTask
export const CreateTask201 = CreatedTask
export type GetTaskParams = { readonly "branch"?: string }
export const GetTaskParams = Schema.Struct({ "branch": Schema.optionalKey(Schema.String) })
export type GetTask200 = TaskDetail
export const GetTask200 = TaskDetail
export type GetTask404 = ApiErrorBody
export const GetTask404 = ApiErrorBody
export type DeleteTaskParams = { readonly "branch"?: string }
export const DeleteTaskParams = Schema.Struct({ "branch": Schema.optionalKey(Schema.String) })
export type DeleteTask409 = ApiErrorBody
export const DeleteTask409 = ApiErrorBody
export type PatchTaskParams = { readonly "branch"?: string }
export const PatchTaskParams = Schema.Struct({ "branch": Schema.optionalKey(Schema.String) })
export type PatchTaskRequestJson = TaskPatch
export const PatchTaskRequestJson = TaskPatch
export type PatchTask200 = TaskDetail
export const PatchTask200 = TaskDetail
export type PatchTask404 = ApiErrorBody
export const PatchTask404 = ApiErrorBody
export type PatchTask409 = ApiErrorBody
export const PatchTask409 = ApiErrorBody
export type Health200 = DaemonInfo
export const Health200 = DaemonInfo

export interface OperationConfig {
  /**
   * Whether or not the response should be included in the value returned from
   * an operation.
   *
   * If set to `true`, a tuple of `[A, HttpClientResponse]` will be returned,
   * where `A` is the success type of the operation.
   *
   * If set to `false`, only the success type of the operation will be returned.
   */
  readonly includeResponse?: boolean | undefined
}

/**
 * A utility type which optionally includes the response in the return result
 * of an operation based upon the value of the `includeResponse` configuration
 * option.
 */
export type WithOptionalResponse<A, Config extends OperationConfig> = Config extends {
  readonly includeResponse: true
} ? [A, HttpClientResponse.HttpClientResponse]
  : A

export const make = (
  httpClient: HttpClient.HttpClient,
  options: {
    readonly transformClient?: ((client: HttpClient.HttpClient) => Effect.Effect<HttpClient.HttpClient>) | undefined
  } = {},
): TasksClient => {
  const unexpectedStatus = (response: HttpClientResponse.HttpClientResponse) =>
    Effect.flatMap(
      Effect.orElseSucceed(response.json, () => "Unexpected status code"),
      (description) =>
        Effect.fail(
          new HttpClientError.HttpClientError({
            reason: new HttpClientError.StatusCodeError({
              request: response.request,
              response,
              description: typeof description === "string" ? description : JSON.stringify(description),
            }),
          }),
        ),
    )
  const withResponse = <Config extends OperationConfig>(config: Config | undefined) =>
  (
    f: (response: HttpClientResponse.HttpClientResponse) => Effect.Effect<any, any>,
  ): (request: HttpClientRequest.HttpClientRequest) => Effect.Effect<any, any> => {
    const withOptionalResponse = (
      config?.includeResponse
        ? (response: HttpClientResponse.HttpClientResponse) => Effect.map(f(response), (a) => [a, response])
        : (response: HttpClientResponse.HttpClientResponse) => f(response)
    ) as any
    return options?.transformClient
      ? (request) =>
        Effect.flatMap(
          Effect.flatMap(options.transformClient!(httpClient), (client) => client.execute(request)),
          withOptionalResponse,
        )
      : (request) => Effect.flatMap(httpClient.execute(request), withOptionalResponse)
  }
  const decodeSuccess =
    <Schema extends Schema.Constraint>(schema: Schema) => (response: HttpClientResponse.HttpClientResponse) =>
      HttpClientResponse.schemaBodyJson(schema)(response)
  const decodeError =
    <const Tag extends string, Schema extends Schema.Constraint>(tag: Tag, schema: Schema) =>
    (response: HttpClientResponse.HttpClientResponse) =>
      Effect.flatMap(
        HttpClientResponse.schemaBodyJson(schema)(response),
        (cause) => Effect.fail(TasksClientError(tag, cause, response)),
      )
  return {
    httpClient,
    "listTasks": (options) =>
      HttpClientRequest.get(`/api/tasks`).pipe(
        withResponse(options?.config)(HttpClientResponse.matchStatus({
          "2xx": decodeSuccess(ListTasks200),
          orElse: unexpectedStatus,
        })),
      ),
    "createTask": (options) =>
      HttpClientRequest.post(`/api/tasks`).pipe(
        HttpClientRequest.bodyJsonUnsafe(options.payload),
        withResponse(options.config)(HttpClientResponse.matchStatus({
          "2xx": decodeSuccess(CreateTask201),
          orElse: unexpectedStatus,
        })),
      ),
    "getTask": (id, options) =>
      HttpClientRequest.get(`/api/tasks/${id}`).pipe(
        HttpClientRequest.setUrlParams({ "branch": options?.params?.["branch"] as any }),
        withResponse(options?.config)(HttpClientResponse.matchStatus({
          "2xx": decodeSuccess(GetTask200),
          "404": decodeError("GetTask404", GetTask404),
          orElse: unexpectedStatus,
        })),
      ),
    "deleteTask": (id, options) =>
      HttpClientRequest.delete(`/api/tasks/${id}`).pipe(
        HttpClientRequest.setUrlParams({ "branch": options?.params?.["branch"] as any }),
        withResponse(options?.config)(HttpClientResponse.matchStatus({
          "409": decodeError("DeleteTask409", DeleteTask409),
          "204": () => Effect.void,
          orElse: unexpectedStatus,
        })),
      ),
    "patchTask": (id, options) =>
      HttpClientRequest.patch(`/api/tasks/${id}`).pipe(
        HttpClientRequest.setUrlParams({ "branch": options.params?.["branch"] as any }),
        HttpClientRequest.bodyJsonUnsafe(options.payload),
        withResponse(options.config)(HttpClientResponse.matchStatus({
          "2xx": decodeSuccess(PatchTask200),
          "404": decodeError("PatchTask404", PatchTask404),
          "409": decodeError("PatchTask409", PatchTask409),
          orElse: unexpectedStatus,
        })),
      ),
    "health": (options) =>
      HttpClientRequest.get(`/health`).pipe(
        withResponse(options?.config)(HttpClientResponse.matchStatus({
          "2xx": decodeSuccess(Health200),
          orElse: unexpectedStatus,
        })),
      ),
  }
}

export interface TasksClient {
  readonly httpClient: HttpClient.HttpClient
  readonly "listTasks": <Config extends OperationConfig>(
    options: { readonly config?: Config | undefined } | undefined,
  ) => Effect.Effect<
    WithOptionalResponse<typeof ListTasks200.Type, Config>,
    HttpClientError.HttpClientError | SchemaError
  >
  readonly "createTask": <Config extends OperationConfig>(
    options: { readonly payload: typeof CreateTaskRequestJson.Encoded; readonly config?: Config | undefined },
  ) => Effect.Effect<
    WithOptionalResponse<typeof CreateTask201.Type, Config>,
    HttpClientError.HttpClientError | SchemaError
  >
  readonly "getTask": <Config extends OperationConfig>(
    id: string,
    options:
      | { readonly params?: typeof GetTaskParams.Encoded | undefined; readonly config?: Config | undefined }
      | undefined,
  ) => Effect.Effect<
    WithOptionalResponse<typeof GetTask200.Type, Config>,
    HttpClientError.HttpClientError | SchemaError | TasksClientError<"GetTask404", typeof GetTask404.Type>
  >
  readonly "deleteTask": <Config extends OperationConfig>(
    id: string,
    options:
      | { readonly params?: typeof DeleteTaskParams.Encoded | undefined; readonly config?: Config | undefined }
      | undefined,
  ) => Effect.Effect<
    WithOptionalResponse<void, Config>,
    HttpClientError.HttpClientError | SchemaError | TasksClientError<"DeleteTask409", typeof DeleteTask409.Type>
  >
  readonly "patchTask": <Config extends OperationConfig>(
    id: string,
    options: {
      readonly params?: typeof PatchTaskParams.Encoded | undefined
      readonly payload: typeof PatchTaskRequestJson.Encoded
      readonly config?: Config | undefined
    },
  ) => Effect.Effect<
    WithOptionalResponse<typeof PatchTask200.Type, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"PatchTask404", typeof PatchTask404.Type>
    | TasksClientError<"PatchTask409", typeof PatchTask409.Type>
  >
  readonly "health": <Config extends OperationConfig>(
    options: { readonly config?: Config | undefined } | undefined,
  ) => Effect.Effect<WithOptionalResponse<typeof Health200.Type, Config>, HttpClientError.HttpClientError | SchemaError>
}

export interface TasksClientError<Tag extends string, E> {
  readonly _tag: Tag
  readonly request: HttpClientRequest.HttpClientRequest
  readonly response: HttpClientResponse.HttpClientResponse
  readonly cause: E
}

class TasksClientErrorImpl extends Data.Error<{
  _tag: string
  cause: any
  request: HttpClientRequest.HttpClientRequest
  response: HttpClientResponse.HttpClientResponse
}> {}

export const TasksClientError = <Tag extends string, E>(
  tag: Tag,
  cause: E,
  response: HttpClientResponse.HttpClientResponse,
): TasksClientError<Tag, E> =>
  new TasksClientErrorImpl({
    _tag: tag,
    cause,
    response,
    request: response.request,
  }) as any
