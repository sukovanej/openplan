import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import type { SchemaError } from "effect/Schema"
import * as Schema from "effect/Schema"
import type * as HttpClient from "effect/unstable/http/HttpClient"
import * as HttpClientError from "effect/unstable/http/HttpClientError"
import * as HttpClientRequest from "effect/unstable/http/HttpClientRequest"
import * as HttpClientResponse from "effect/unstable/http/HttpClientResponse"
// non-recursive definitions
export type ApiErrorBody = { readonly message: string }
export const ApiErrorBody = Schema.Struct({ message: Schema.String })
export type ChangeKind = "base" | "added" | "modified" | "deleted"
export const ChangeKind = Schema.Literals(["base", "added", "modified", "deleted"])
export type CreatedTask = { readonly id: string }
export const CreatedTask = Schema.Struct({ id: Schema.String })
export type DaemonInfo = {
  readonly pid: number
  readonly port: number
  readonly started_at: number
  readonly version: string
}
export const DaemonInfo = Schema.Struct({
  pid: Schema.Number.annotate({ format: "int32" }).check(Schema.isInt()).check(Schema.isGreaterThanOrEqualTo(0)),
  port: Schema.Number.annotate({ format: "int32" }).check(Schema.isInt()).check(Schema.isGreaterThanOrEqualTo(0)),
  started_at: Schema.Number.annotate({ format: "int64" }).check(Schema.isInt()).check(Schema.isGreaterThanOrEqualTo(0)),
  version: Schema.String,
})
export type Status = "backlog" | "todo" | "in_progress" | "in_review" | "done" | "cancelled"
export const Status = Schema.Literals(["backlog", "todo", "in_progress", "in_review", "done", "cancelled"])
export type BranchState = {
  readonly blob_oid: string
  readonly branch: string
  readonly dirty: boolean
  readonly kind: ChangeKind
  readonly status: Status
}
export const BranchState = Schema.Struct({
  blob_oid: Schema.String,
  branch: Schema.String,
  dirty: Schema.Boolean,
  kind: ChangeKind,
  status: Status,
})
export type CreateTask = {
  readonly body?: string | null
  readonly deps?: ReadonlyArray<string>
  readonly parent?: string | null
  readonly status?: null | Status
  readonly title: string
}
export const CreateTask = Schema.Struct({
  body: Schema.optionalKey(Schema.Union([Schema.String, Schema.Null])),
  deps: Schema.optionalKey(Schema.Array(Schema.String)),
  parent: Schema.optionalKey(Schema.Union([Schema.String, Schema.Null])),
  status: Schema.optionalKey(Schema.Union([Schema.Null, Status], { mode: "oneOf" })),
  title: Schema.String,
})
export type TaskChild = { readonly id: string; readonly rank?: string; readonly status: Status; readonly title: string }
export const TaskChild = Schema.Struct({
  id: Schema.String,
  rank: Schema.optionalKey(Schema.String),
  status: Status,
  title: Schema.String,
})
export type TaskPatch = {
  readonly deps?: ReadonlyArray<string>
  readonly parent?: string | null
  readonly rank?: string
  readonly status?: Status
}
export const TaskPatch = Schema.Struct({
  deps: Schema.optionalKey(Schema.Array(Schema.String)),
  parent: Schema.optionalKey(Schema.Union([Schema.String, Schema.Null])),
  rank: Schema.optionalKey(Schema.String),
  status: Schema.optionalKey(Status),
})
export type TaskRef = { readonly id: string; readonly status: Status; readonly title: string }
export const TaskRef = Schema.Struct({ id: Schema.String, status: Status, title: Schema.String })
export type TaskListItem = {
  readonly branches: ReadonlyArray<BranchState>
  readonly headline: string
  readonly id: string
  readonly parent?: string
  readonly rank?: string
  readonly status: Status
  readonly title: string
  readonly updated?: string
}
export const TaskListItem = Schema.Struct({
  branches: Schema.Array(BranchState),
  headline: Schema.String,
  id: Schema.String,
  parent: Schema.optionalKey(Schema.String),
  rank: Schema.optionalKey(Schema.String),
  status: Status,
  title: Schema.String,
  updated: Schema.optionalKey(Schema.Union([Schema.String.annotate({ format: "date-time" })])),
})
export type TaskDetail = {
  readonly body: string
  readonly created?: string
  readonly deps?: ReadonlyArray<string>
  readonly id: string
  readonly parent?: string
  readonly rank?: string
  readonly status: Status
  readonly title: string
  readonly updated?: string
  readonly branches: ReadonlyArray<BranchState>
  readonly children?: ReadonlyArray<TaskChild>
  readonly headline: string
  readonly parent_title?: string
  readonly refs?: ReadonlyArray<TaskRef>
}
export const TaskDetail = Schema.Struct({
  body: Schema.String,
  created: Schema.optionalKey(Schema.Union([Schema.String.annotate({ format: "date-time" })])),
  deps: Schema.optionalKey(Schema.Array(Schema.String)),
  id: Schema.String,
  parent: Schema.optionalKey(Schema.String),
  rank: Schema.optionalKey(Schema.String),
  status: Status,
  title: Schema.String,
  updated: Schema.optionalKey(Schema.Union([Schema.String.annotate({ format: "date-time" })])),
  branches: Schema.Array(BranchState),
  children: Schema.optionalKey(Schema.Array(TaskChild)),
  headline: Schema.String,
  parent_title: Schema.optionalKey(Schema.String),
  refs: Schema.optionalKey(Schema.Array(TaskRef)),
})
export type BoardRow = {
  readonly depth: number
  readonly has_children: boolean
  readonly parent_title?: string
  readonly task: TaskListItem
}
export const BoardRow = Schema.Struct({
  depth: Schema.Number.check(Schema.isInt()).check(Schema.isGreaterThanOrEqualTo(0)),
  has_children: Schema.Boolean,
  parent_title: Schema.optionalKey(Schema.String),
  task: TaskListItem,
})
export type BoardGroup = { readonly rows: ReadonlyArray<BoardRow>; readonly status: Status }
export const BoardGroup = Schema.Struct({ rows: Schema.Array(BoardRow), status: Status })
export type Board = { readonly groups: ReadonlyArray<BoardGroup> }
export const Board = Schema.Struct({ groups: Schema.Array(BoardGroup) })
// schemas
export type GetBoard200 = Board
export const GetBoard200 = Board
export type GetBoard400 = ApiErrorBody
export const GetBoard400 = ApiErrorBody
export type GetBoard500 = ApiErrorBody
export const GetBoard500 = ApiErrorBody
export type ListTasks200 = ReadonlyArray<TaskListItem>
export const ListTasks200 = Schema.Array(TaskListItem)
export type ListTasks400 = ApiErrorBody
export const ListTasks400 = ApiErrorBody
export type ListTasks500 = ApiErrorBody
export const ListTasks500 = ApiErrorBody
export type CreateTaskRequestJson = CreateTask
export const CreateTaskRequestJson = CreateTask
export type CreateTask201 = CreatedTask
export const CreateTask201 = CreatedTask
export type CreateTask400 = ApiErrorBody
export const CreateTask400 = ApiErrorBody
export type CreateTask500 = ApiErrorBody
export const CreateTask500 = ApiErrorBody
export type GetTaskParams = { readonly branch?: string }
export const GetTaskParams = Schema.Struct({ branch: Schema.optionalKey(Schema.String) })
export type GetTask200 = TaskDetail
export const GetTask200 = TaskDetail
export type GetTask400 = ApiErrorBody
export const GetTask400 = ApiErrorBody
export type GetTask404 = ApiErrorBody
export const GetTask404 = ApiErrorBody
export type GetTask500 = ApiErrorBody
export const GetTask500 = ApiErrorBody
export type DeleteTaskParams = { readonly branch?: string }
export const DeleteTaskParams = Schema.Struct({ branch: Schema.optionalKey(Schema.String) })
export type DeleteTask400 = ApiErrorBody
export const DeleteTask400 = ApiErrorBody
export type DeleteTask404 = ApiErrorBody
export const DeleteTask404 = ApiErrorBody
export type DeleteTask409 = ApiErrorBody
export const DeleteTask409 = ApiErrorBody
export type DeleteTask500 = ApiErrorBody
export const DeleteTask500 = ApiErrorBody
export type PatchTaskParams = { readonly branch?: string }
export const PatchTaskParams = Schema.Struct({ branch: Schema.optionalKey(Schema.String) })
export type PatchTaskRequestJson = TaskPatch
export const PatchTaskRequestJson = TaskPatch
export type PatchTask200 = TaskDetail
export const PatchTask200 = TaskDetail
export type PatchTask400 = ApiErrorBody
export const PatchTask400 = ApiErrorBody
export type PatchTask404 = ApiErrorBody
export const PatchTask404 = ApiErrorBody
export type PatchTask409 = ApiErrorBody
export const PatchTask409 = ApiErrorBody
export type PatchTask500 = ApiErrorBody
export const PatchTask500 = ApiErrorBody
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
}
  ? [A, HttpClientResponse.HttpClientResponse]
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
  const withResponse =
    <Config extends OperationConfig>(config: Config | undefined) =>
    (
      f: (response: HttpClientResponse.HttpClientResponse) => Effect.Effect<any, any>,
    ): ((request: HttpClientRequest.HttpClientRequest) => Effect.Effect<any, any>) => {
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
    <Schema extends Schema.Constraint>(schema: Schema) =>
    (response: HttpClientResponse.HttpClientResponse) =>
      HttpClientResponse.schemaBodyJson(schema)(response)
  const decodeError =
    <const Tag extends string, Schema extends Schema.Constraint>(tag: Tag, schema: Schema) =>
    (response: HttpClientResponse.HttpClientResponse) =>
      Effect.flatMap(HttpClientResponse.schemaBodyJson(schema)(response), (cause) =>
        Effect.fail(TasksClientError(tag, cause, response)),
      )
  return {
    httpClient,
    getBoard: (options) =>
      HttpClientRequest.get(`/api/board`).pipe(
        withResponse(options?.config)(
          HttpClientResponse.matchStatus({
            "2xx": decodeSuccess(GetBoard200),
            "400": decodeError("GetBoard400", GetBoard400),
            "500": decodeError("GetBoard500", GetBoard500),
            orElse: unexpectedStatus,
          }),
        ),
      ),
    listTasks: (options) =>
      HttpClientRequest.get(`/api/tasks`).pipe(
        withResponse(options?.config)(
          HttpClientResponse.matchStatus({
            "2xx": decodeSuccess(ListTasks200),
            "400": decodeError("ListTasks400", ListTasks400),
            "500": decodeError("ListTasks500", ListTasks500),
            orElse: unexpectedStatus,
          }),
        ),
      ),
    createTask: (options) =>
      HttpClientRequest.post(`/api/tasks`).pipe(
        HttpClientRequest.bodyJsonUnsafe(options.payload),
        withResponse(options.config)(
          HttpClientResponse.matchStatus({
            "2xx": decodeSuccess(CreateTask201),
            "400": decodeError("CreateTask400", CreateTask400),
            "500": decodeError("CreateTask500", CreateTask500),
            orElse: unexpectedStatus,
          }),
        ),
      ),
    getTask: (id, options) =>
      HttpClientRequest.get(`/api/tasks/${id}`).pipe(
        HttpClientRequest.setUrlParams({ branch: options?.params?.["branch"] as any }),
        withResponse(options?.config)(
          HttpClientResponse.matchStatus({
            "2xx": decodeSuccess(GetTask200),
            "400": decodeError("GetTask400", GetTask400),
            "404": decodeError("GetTask404", GetTask404),
            "500": decodeError("GetTask500", GetTask500),
            orElse: unexpectedStatus,
          }),
        ),
      ),
    deleteTask: (id, options) =>
      HttpClientRequest.delete(`/api/tasks/${id}`).pipe(
        HttpClientRequest.setUrlParams({ branch: options?.params?.["branch"] as any }),
        withResponse(options?.config)(
          HttpClientResponse.matchStatus({
            "400": decodeError("DeleteTask400", DeleteTask400),
            "404": decodeError("DeleteTask404", DeleteTask404),
            "409": decodeError("DeleteTask409", DeleteTask409),
            "500": decodeError("DeleteTask500", DeleteTask500),
            "204": () => Effect.void,
            orElse: unexpectedStatus,
          }),
        ),
      ),
    patchTask: (id, options) =>
      HttpClientRequest.patch(`/api/tasks/${id}`).pipe(
        HttpClientRequest.setUrlParams({ branch: options.params?.["branch"] as any }),
        HttpClientRequest.bodyJsonUnsafe(options.payload),
        withResponse(options.config)(
          HttpClientResponse.matchStatus({
            "2xx": decodeSuccess(PatchTask200),
            "400": decodeError("PatchTask400", PatchTask400),
            "404": decodeError("PatchTask404", PatchTask404),
            "409": decodeError("PatchTask409", PatchTask409),
            "500": decodeError("PatchTask500", PatchTask500),
            orElse: unexpectedStatus,
          }),
        ),
      ),
    health: (options) =>
      HttpClientRequest.get(`/health`).pipe(
        withResponse(options?.config)(
          HttpClientResponse.matchStatus({
            "2xx": decodeSuccess(Health200),
            orElse: unexpectedStatus,
          }),
        ),
      ),
  }
}

export interface TasksClient {
  readonly httpClient: HttpClient.HttpClient
  readonly getBoard: <Config extends OperationConfig>(
    options: { readonly config?: Config | undefined } | undefined,
  ) => Effect.Effect<
    WithOptionalResponse<typeof GetBoard200.Type, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"GetBoard400", typeof GetBoard400.Type>
    | TasksClientError<"GetBoard500", typeof GetBoard500.Type>
  >
  readonly listTasks: <Config extends OperationConfig>(
    options: { readonly config?: Config | undefined } | undefined,
  ) => Effect.Effect<
    WithOptionalResponse<typeof ListTasks200.Type, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"ListTasks400", typeof ListTasks400.Type>
    | TasksClientError<"ListTasks500", typeof ListTasks500.Type>
  >
  readonly createTask: <Config extends OperationConfig>(options: {
    readonly payload: typeof CreateTaskRequestJson.Encoded
    readonly config?: Config | undefined
  }) => Effect.Effect<
    WithOptionalResponse<typeof CreateTask201.Type, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"CreateTask400", typeof CreateTask400.Type>
    | TasksClientError<"CreateTask500", typeof CreateTask500.Type>
  >
  readonly getTask: <Config extends OperationConfig>(
    id: string,
    options:
      | { readonly params?: typeof GetTaskParams.Encoded | undefined; readonly config?: Config | undefined }
      | undefined,
  ) => Effect.Effect<
    WithOptionalResponse<typeof GetTask200.Type, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"GetTask400", typeof GetTask400.Type>
    | TasksClientError<"GetTask404", typeof GetTask404.Type>
    | TasksClientError<"GetTask500", typeof GetTask500.Type>
  >
  readonly deleteTask: <Config extends OperationConfig>(
    id: string,
    options:
      | { readonly params?: typeof DeleteTaskParams.Encoded | undefined; readonly config?: Config | undefined }
      | undefined,
  ) => Effect.Effect<
    WithOptionalResponse<void, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"DeleteTask400", typeof DeleteTask400.Type>
    | TasksClientError<"DeleteTask404", typeof DeleteTask404.Type>
    | TasksClientError<"DeleteTask409", typeof DeleteTask409.Type>
    | TasksClientError<"DeleteTask500", typeof DeleteTask500.Type>
  >
  readonly patchTask: <Config extends OperationConfig>(
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
    | TasksClientError<"PatchTask400", typeof PatchTask400.Type>
    | TasksClientError<"PatchTask404", typeof PatchTask404.Type>
    | TasksClientError<"PatchTask409", typeof PatchTask409.Type>
    | TasksClientError<"PatchTask500", typeof PatchTask500.Type>
  >
  readonly health: <Config extends OperationConfig>(
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
