import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import type { SchemaError } from "effect/Schema"
import * as Schema from "effect/Schema"
import type * as HttpClient from "effect/unstable/http/HttpClient"
import * as HttpClientError from "effect/unstable/http/HttpClientError"
import * as HttpClientRequest from "effect/unstable/http/HttpClientRequest"
import * as HttpClientResponse from "effect/unstable/http/HttpClientResponse"
// recursive declarations
export type TaskTree = {
  readonly children: ReadonlyArray<TaskTree>
  readonly id: string
  readonly metadata: Metadata
  readonly title: string
}
export const TaskTree = Schema.suspend((): Schema.Codec<TaskTree> => __recursive_TaskTree)
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
export type FieldError = { readonly kind: "missing" } | { readonly kind: "invalid"; readonly message: string }
export const FieldError = Schema.Union(
  [
    Schema.Struct({ kind: Schema.Literal("missing") }),
    Schema.Struct({ kind: Schema.Literal("invalid"), message: Schema.String }),
  ],
  { mode: "oneOf" },
)
export type MetadataErrorTag = "error"
export const MetadataErrorTag = Schema.Literal("error")
export type ProjectStatus = { readonly state: "ok" } | { readonly reason: string; readonly state: "error" }
export const ProjectStatus = Schema.Union(
  [
    Schema.Struct({ state: Schema.Literal("ok") }),
    Schema.Struct({ reason: Schema.String, state: Schema.Literal("error") }),
  ],
  { mode: "oneOf" },
)
export type RegisterProject = { readonly path: string }
export const RegisterProject = Schema.Struct({ path: Schema.String })
export type RenameProject = { readonly name: string }
export const RenameProject = Schema.Struct({ name: Schema.String })
export type Status = "backlog" | "todo" | "in_progress" | "in_review" | "done" | "cancelled"
export const Status = Schema.Literals(["backlog", "todo", "in_progress", "in_review", "done", "cancelled"])
export type StoreConfig = { readonly abbreviation: string }
export const StoreConfig = Schema.Struct({
  abbreviation: Schema.String.check(Schema.isPattern(new RegExp("^[A-Z]{3}$"))),
})
export type TaskTreeView = { readonly cycles?: ReadonlyArray<string>; readonly tree: TaskTree }
export const TaskTreeView = Schema.Struct({ cycles: Schema.optionalKey(Schema.Array(Schema.String)), tree: TaskTree })
export type BranchMark = { readonly branch: string; readonly dirty: boolean; readonly kind: ChangeKind }
export const BranchMark = Schema.Struct({ branch: Schema.String, dirty: Schema.Boolean, kind: ChangeKind })
export type Field_Option_String = null | string | FieldError
export const Field_Option_String = Schema.Union(
  [Schema.Union([Schema.Null, Schema.String], { mode: "oneOf" }), FieldError],
  { mode: "oneOf" },
)
export type Field_Rfc3339 = string | FieldError
export const Field_Rfc3339 = Schema.Union([Schema.String.annotate({ format: "date-time" }), FieldError], {
  mode: "oneOf",
})
export type Field_Status = "backlog" | "todo" | "in_progress" | "in_review" | "done" | "cancelled" | FieldError
export const Field_Status = Schema.Union(
  [Schema.Literals(["backlog", "todo", "in_progress", "in_review", "done", "cancelled"]), FieldError],
  { mode: "oneOf" },
)
export type Field_Vec_String = ReadonlyArray<string> | FieldError
export const Field_Vec_String = Schema.Union([Schema.Array(Schema.String), FieldError], { mode: "oneOf" })
export type ProjectView = {
  readonly abbreviation: string
  readonly git_common_dir: string
  readonly name: string
  readonly root: string
  readonly status: ProjectStatus
}
export const ProjectView = Schema.Struct({
  abbreviation: Schema.String,
  git_common_dir: Schema.String,
  name: Schema.String,
  root: Schema.String,
  status: ProjectStatus,
})
export type CreateTask = {
  readonly body?: string | null
  readonly dependencies?: ReadonlyArray<string>
  readonly parent?: string | null
  readonly status?: null | Status
  readonly title: string
}
export const CreateTask = Schema.Struct({
  body: Schema.optionalKey(Schema.Union([Schema.String, Schema.Null])),
  dependencies: Schema.optionalKey(Schema.Array(Schema.String)),
  parent: Schema.optionalKey(Schema.Union([Schema.String, Schema.Null])),
  status: Schema.optionalKey(Schema.Union([Schema.Null, Status], { mode: "oneOf" })),
  title: Schema.String,
})
export type TaskPatch = {
  readonly dependencies?: ReadonlyArray<string>
  readonly parent?: string | null
  readonly rank?: string
  readonly status?: Status
}
export const TaskPatch = Schema.Struct({
  dependencies: Schema.optionalKey(Schema.Array(Schema.String)),
  parent: Schema.optionalKey(Schema.Union([Schema.String, Schema.Null])),
  rank: Schema.optionalKey(Schema.String),
  status: Schema.optionalKey(Status),
})
export type BranchState = {
  readonly blob_oid: string
  readonly branch: string
  readonly dirty: boolean
  readonly kind: ChangeKind
  readonly status: Field_Status
}
export const BranchState = Schema.Struct({
  blob_oid: Schema.String,
  branch: Schema.String,
  dirty: Schema.Boolean,
  kind: ChangeKind,
  status: Field_Status,
})
export type TaskChild = {
  readonly id: string
  readonly rank?: string
  readonly status: Field_Status
  readonly title: string
}
export const TaskChild = Schema.Struct({
  id: Schema.String,
  rank: Schema.optionalKey(Schema.String),
  status: Field_Status,
  title: Schema.String,
})
export type TaskRef = { readonly id: string; readonly status: Field_Status; readonly title: string }
export const TaskRef = Schema.Struct({ id: Schema.String, status: Field_Status, title: Schema.String })
export type FrontmatterFields = {
  readonly created: Field_Rfc3339
  readonly dependencies: Field_Vec_String
  readonly parent: Field_Option_String
  readonly rank: Field_Option_String
  readonly status: Field_Status
}
export const FrontmatterFields = Schema.Struct({
  created: Field_Rfc3339,
  dependencies: Field_Vec_String,
  parent: Field_Option_String,
  rank: Field_Option_String,
  status: Field_Status,
})
export type Metadata = { readonly kind: MetadataErrorTag; readonly message: string } | FrontmatterFields
export const Metadata = Schema.Union(
  [Schema.Struct({ kind: MetadataErrorTag, message: Schema.String }), FrontmatterFields],
  { mode: "oneOf" },
)
export type TaskDetail = {
  readonly body: string
  readonly branches: ReadonlyArray<BranchState>
  readonly children?: ReadonlyArray<TaskChild>
  readonly headline: string
  readonly id: string
  readonly metadata: Metadata
  readonly parent_title?: string
  readonly project: string
  readonly refs?: ReadonlyArray<TaskRef>
  readonly title: string
  readonly updated: Field_Rfc3339
}
export const TaskDetail = Schema.Struct({
  body: Schema.String,
  branches: Schema.Array(BranchState),
  children: Schema.optionalKey(Schema.Array(TaskChild)),
  headline: Schema.String,
  id: Schema.String,
  metadata: Metadata,
  parent_title: Schema.optionalKey(Schema.String),
  project: Schema.String,
  refs: Schema.optionalKey(Schema.Array(TaskRef)),
  title: Schema.String,
  updated: Field_Rfc3339,
})
export type TaskListItem = {
  readonly branches: ReadonlyArray<BranchState>
  readonly headline: string
  readonly id: string
  readonly metadata: Metadata
  readonly project: string
  readonly title: string
  readonly updated: Field_Rfc3339
}
export const TaskListItem = Schema.Struct({
  branches: Schema.Array(BranchState),
  headline: Schema.String,
  id: Schema.String,
  metadata: Metadata,
  project: Schema.String,
  title: Schema.String,
  updated: Field_Rfc3339,
})
export type TaskSummary = { readonly id: string; readonly metadata: Metadata; readonly title: string }
export const TaskSummary = Schema.Struct({ id: Schema.String, metadata: Metadata, title: Schema.String })
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
export type MatrixCell = {
  readonly blob_oid: string
  readonly branch: string
  readonly dirty: boolean
  readonly kind: ChangeKind
  readonly task: TaskSummary
}
export const MatrixCell = Schema.Struct({
  blob_oid: Schema.String,
  branch: Schema.String,
  dirty: Schema.Boolean,
  kind: ChangeKind,
  task: TaskSummary,
})
export type TaskVersion = {
  readonly blob_oid: string
  readonly branches: ReadonlyArray<BranchMark>
  readonly summary: TaskSummary
}
export const TaskVersion = Schema.Struct({
  blob_oid: Schema.String,
  branches: Schema.Array(BranchMark),
  summary: TaskSummary,
})
export type BoardGroup = { readonly rows: ReadonlyArray<BoardRow>; readonly status?: Status }
export const BoardGroup = Schema.Struct({ rows: Schema.Array(BoardRow), status: Schema.optionalKey(Status) })
export type Matrix = { readonly cells: ReadonlyArray<MatrixCell> }
export const Matrix = Schema.Struct({ cells: Schema.Array(MatrixCell) })
export type TaskBranches = { readonly id: string; readonly versions: ReadonlyArray<TaskVersion> }
export const TaskBranches = Schema.Struct({ id: Schema.String, versions: Schema.Array(TaskVersion) })
export type Board = { readonly groups: ReadonlyArray<BoardGroup> }
export const Board = Schema.Struct({ groups: Schema.Array(BoardGroup) })
// recursive definitions
const __recursive_TaskTree = Schema.Struct({
  children: Schema.Array(Schema.suspend((): Schema.Codec<TaskTree> => TaskTree)),
  id: Schema.String,
  metadata: Metadata,
  title: Schema.String,
})
// schemas
export type GetMergedBoard200 = Board
export const GetMergedBoard200 = Board
export type GetMergedBoard500 = ApiErrorBody
export const GetMergedBoard500 = ApiErrorBody
export type ListProjects200 = ReadonlyArray<ProjectView>
export const ListProjects200 = Schema.Array(ProjectView)
export type RegisterProjectRequestJson = RegisterProject
export const RegisterProjectRequestJson = RegisterProject
export type RegisterProject200 = ProjectView
export const RegisterProject200 = ProjectView
export type RegisterProject201 = ProjectView
export const RegisterProject201 = ProjectView
export type RegisterProject400 = ApiErrorBody
export const RegisterProject400 = ApiErrorBody
export type RegisterProject503 = ApiErrorBody
export const RegisterProject503 = ApiErrorBody
export type DeleteProject404 = ApiErrorBody
export const DeleteProject404 = ApiErrorBody
export type DeleteProject503 = ApiErrorBody
export const DeleteProject503 = ApiErrorBody
export type RenameProjectRequestJson = RenameProject
export const RenameProjectRequestJson = RenameProject
export type RenameProject200 = ProjectView
export const RenameProject200 = ProjectView
export type RenameProject400 = ApiErrorBody
export const RenameProject400 = ApiErrorBody
export type RenameProject404 = ApiErrorBody
export const RenameProject404 = ApiErrorBody
export type RenameProject409 = ApiErrorBody
export const RenameProject409 = ApiErrorBody
export type RenameProject503 = ApiErrorBody
export const RenameProject503 = ApiErrorBody
export type GetBoard200 = Board
export const GetBoard200 = Board
export type GetBoard400 = ApiErrorBody
export const GetBoard400 = ApiErrorBody
export type GetBoard404 = ApiErrorBody
export const GetBoard404 = ApiErrorBody
export type GetBoard500 = ApiErrorBody
export const GetBoard500 = ApiErrorBody
export type GetBoard503 = ApiErrorBody
export const GetBoard503 = ApiErrorBody
export type GetConfig200 = StoreConfig
export const GetConfig200 = StoreConfig
export type GetConfig404 = ApiErrorBody
export const GetConfig404 = ApiErrorBody
export type GetConfig503 = ApiErrorBody
export const GetConfig503 = ApiErrorBody
export type GetMatrixParams = { readonly fresh?: boolean }
export const GetMatrixParams = Schema.Struct({ fresh: Schema.optionalKey(Schema.Boolean) })
export type GetMatrix200 = Matrix
export const GetMatrix200 = Matrix
export type GetMatrix404 = ApiErrorBody
export const GetMatrix404 = ApiErrorBody
export type GetMatrix500 = ApiErrorBody
export const GetMatrix500 = ApiErrorBody
export type GetMatrix503 = ApiErrorBody
export const GetMatrix503 = ApiErrorBody
export type ListTasksParams = { readonly branch?: string | null; readonly fresh?: boolean }
export const ListTasksParams = Schema.Struct({
  branch: Schema.optionalKey(Schema.Union([Schema.String, Schema.Null])),
  fresh: Schema.optionalKey(Schema.Boolean),
})
export type ListTasks200 = ReadonlyArray<TaskListItem>
export const ListTasks200 = Schema.Array(TaskListItem)
export type ListTasks400 = ApiErrorBody
export const ListTasks400 = ApiErrorBody
export type ListTasks404 = ApiErrorBody
export const ListTasks404 = ApiErrorBody
export type ListTasks500 = ApiErrorBody
export const ListTasks500 = ApiErrorBody
export type ListTasks503 = ApiErrorBody
export const ListTasks503 = ApiErrorBody
export type CreateTaskParams = { readonly branch?: string }
export const CreateTaskParams = Schema.Struct({ branch: Schema.optionalKey(Schema.String) })
export type CreateTaskRequestJson = CreateTask
export const CreateTaskRequestJson = CreateTask
export type CreateTask201 = CreatedTask
export const CreateTask201 = CreatedTask
export type CreateTask400 = ApiErrorBody
export const CreateTask400 = ApiErrorBody
export type CreateTask404 = ApiErrorBody
export const CreateTask404 = ApiErrorBody
export type CreateTask409 = ApiErrorBody
export const CreateTask409 = ApiErrorBody
export type CreateTask500 = ApiErrorBody
export const CreateTask500 = ApiErrorBody
export type CreateTask503 = ApiErrorBody
export const CreateTask503 = ApiErrorBody
export type GetTaskParams = { readonly branch?: string | null; readonly fresh?: boolean }
export const GetTaskParams = Schema.Struct({
  branch: Schema.optionalKey(Schema.Union([Schema.String, Schema.Null])),
  fresh: Schema.optionalKey(Schema.Boolean),
})
export type GetTask200 = TaskDetail
export const GetTask200 = TaskDetail
export type GetTask400 = ApiErrorBody
export const GetTask400 = ApiErrorBody
export type GetTask404 = ApiErrorBody
export const GetTask404 = ApiErrorBody
export type GetTask500 = ApiErrorBody
export const GetTask500 = ApiErrorBody
export type GetTask503 = ApiErrorBody
export const GetTask503 = ApiErrorBody
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
export type DeleteTask503 = ApiErrorBody
export const DeleteTask503 = ApiErrorBody
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
export type PatchTask503 = ApiErrorBody
export const PatchTask503 = ApiErrorBody
export type GetTaskBranchesParams = { readonly fresh?: boolean }
export const GetTaskBranchesParams = Schema.Struct({ fresh: Schema.optionalKey(Schema.Boolean) })
export type GetTaskBranches200 = TaskBranches
export const GetTaskBranches200 = TaskBranches
export type GetTaskBranches400 = ApiErrorBody
export const GetTaskBranches400 = ApiErrorBody
export type GetTaskBranches404 = ApiErrorBody
export const GetTaskBranches404 = ApiErrorBody
export type GetTaskBranches500 = ApiErrorBody
export const GetTaskBranches500 = ApiErrorBody
export type GetTaskBranches503 = ApiErrorBody
export const GetTaskBranches503 = ApiErrorBody
export type GetTaskTreeParams = { readonly branch?: string | null; readonly fresh?: boolean; readonly depth?: number }
export const GetTaskTreeParams = Schema.Struct({
  branch: Schema.optionalKey(Schema.Union([Schema.String, Schema.Null])),
  fresh: Schema.optionalKey(Schema.Boolean),
  depth: Schema.optionalKey(
    Schema.Union([
      Schema.Number.check(Schema.isInt()).check(Schema.isFinite()).check(Schema.isGreaterThanOrEqualTo(0)),
    ]),
  ),
})
export type GetTaskTree200 = TaskTreeView
export const GetTaskTree200 = TaskTreeView
export type GetTaskTree400 = ApiErrorBody
export const GetTaskTree400 = ApiErrorBody
export type GetTaskTree404 = ApiErrorBody
export const GetTaskTree404 = ApiErrorBody
export type GetTaskTree500 = ApiErrorBody
export const GetTaskTree500 = ApiErrorBody
export type GetTaskTree503 = ApiErrorBody
export const GetTaskTree503 = ApiErrorBody
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
    getMergedBoard: (options) =>
      HttpClientRequest.get(`/api/board`).pipe(
        withResponse(options?.config)(
          HttpClientResponse.matchStatus({
            "2xx": decodeSuccess(GetMergedBoard200),
            "500": decodeError("GetMergedBoard500", GetMergedBoard500),
            orElse: unexpectedStatus,
          }),
        ),
      ),
    listProjects: (options) =>
      HttpClientRequest.get(`/api/projects`).pipe(
        withResponse(options?.config)(
          HttpClientResponse.matchStatus({
            "2xx": decodeSuccess(ListProjects200),
            orElse: unexpectedStatus,
          }),
        ),
      ),
    registerProject: (options) =>
      HttpClientRequest.post(`/api/projects`).pipe(
        HttpClientRequest.bodyJsonUnsafe(options.payload),
        withResponse(options.config)(
          HttpClientResponse.matchStatus({
            "200": decodeSuccess(RegisterProject200),
            "201": decodeSuccess(RegisterProject201),
            "400": decodeError("RegisterProject400", RegisterProject400),
            "503": decodeError("RegisterProject503", RegisterProject503),
            orElse: unexpectedStatus,
          }),
        ),
      ),
    deleteProject: (project, options) =>
      HttpClientRequest.delete(`/api/projects/${project}`).pipe(
        withResponse(options?.config)(
          HttpClientResponse.matchStatus({
            "404": decodeError("DeleteProject404", DeleteProject404),
            "503": decodeError("DeleteProject503", DeleteProject503),
            "204": () => Effect.void,
            orElse: unexpectedStatus,
          }),
        ),
      ),
    renameProject: (project, options) =>
      HttpClientRequest.patch(`/api/projects/${project}`).pipe(
        HttpClientRequest.bodyJsonUnsafe(options.payload),
        withResponse(options.config)(
          HttpClientResponse.matchStatus({
            "2xx": decodeSuccess(RenameProject200),
            "400": decodeError("RenameProject400", RenameProject400),
            "404": decodeError("RenameProject404", RenameProject404),
            "409": decodeError("RenameProject409", RenameProject409),
            "503": decodeError("RenameProject503", RenameProject503),
            orElse: unexpectedStatus,
          }),
        ),
      ),
    getBoard: (project, options) =>
      HttpClientRequest.get(`/api/projects/${project}/board`).pipe(
        withResponse(options?.config)(
          HttpClientResponse.matchStatus({
            "2xx": decodeSuccess(GetBoard200),
            "400": decodeError("GetBoard400", GetBoard400),
            "404": decodeError("GetBoard404", GetBoard404),
            "500": decodeError("GetBoard500", GetBoard500),
            "503": decodeError("GetBoard503", GetBoard503),
            orElse: unexpectedStatus,
          }),
        ),
      ),
    getConfig: (project, options) =>
      HttpClientRequest.get(`/api/projects/${project}/config`).pipe(
        withResponse(options?.config)(
          HttpClientResponse.matchStatus({
            "2xx": decodeSuccess(GetConfig200),
            "404": decodeError("GetConfig404", GetConfig404),
            "503": decodeError("GetConfig503", GetConfig503),
            orElse: unexpectedStatus,
          }),
        ),
      ),
    getMatrix: (project, options) =>
      HttpClientRequest.get(`/api/projects/${project}/matrix`).pipe(
        HttpClientRequest.setUrlParams({ fresh: options?.params?.["fresh"] as any }),
        withResponse(options?.config)(
          HttpClientResponse.matchStatus({
            "2xx": decodeSuccess(GetMatrix200),
            "404": decodeError("GetMatrix404", GetMatrix404),
            "500": decodeError("GetMatrix500", GetMatrix500),
            "503": decodeError("GetMatrix503", GetMatrix503),
            orElse: unexpectedStatus,
          }),
        ),
      ),
    listTasks: (project, options) =>
      HttpClientRequest.get(`/api/projects/${project}/tasks`).pipe(
        HttpClientRequest.setUrlParams({
          branch: options?.params?.["branch"] as any,
          fresh: options?.params?.["fresh"] as any,
        }),
        withResponse(options?.config)(
          HttpClientResponse.matchStatus({
            "2xx": decodeSuccess(ListTasks200),
            "400": decodeError("ListTasks400", ListTasks400),
            "404": decodeError("ListTasks404", ListTasks404),
            "500": decodeError("ListTasks500", ListTasks500),
            "503": decodeError("ListTasks503", ListTasks503),
            orElse: unexpectedStatus,
          }),
        ),
      ),
    createTask: (project, options) =>
      HttpClientRequest.post(`/api/projects/${project}/tasks`).pipe(
        HttpClientRequest.setUrlParams({ branch: options.params?.["branch"] as any }),
        HttpClientRequest.bodyJsonUnsafe(options.payload),
        withResponse(options.config)(
          HttpClientResponse.matchStatus({
            "2xx": decodeSuccess(CreateTask201),
            "400": decodeError("CreateTask400", CreateTask400),
            "404": decodeError("CreateTask404", CreateTask404),
            "409": decodeError("CreateTask409", CreateTask409),
            "500": decodeError("CreateTask500", CreateTask500),
            "503": decodeError("CreateTask503", CreateTask503),
            orElse: unexpectedStatus,
          }),
        ),
      ),
    getTask: (project, id, options) =>
      HttpClientRequest.get(`/api/projects/${project}/tasks/${id}`).pipe(
        HttpClientRequest.setUrlParams({
          branch: options?.params?.["branch"] as any,
          fresh: options?.params?.["fresh"] as any,
        }),
        withResponse(options?.config)(
          HttpClientResponse.matchStatus({
            "2xx": decodeSuccess(GetTask200),
            "400": decodeError("GetTask400", GetTask400),
            "404": decodeError("GetTask404", GetTask404),
            "500": decodeError("GetTask500", GetTask500),
            "503": decodeError("GetTask503", GetTask503),
            orElse: unexpectedStatus,
          }),
        ),
      ),
    deleteTask: (project, id, options) =>
      HttpClientRequest.delete(`/api/projects/${project}/tasks/${id}`).pipe(
        HttpClientRequest.setUrlParams({ branch: options?.params?.["branch"] as any }),
        withResponse(options?.config)(
          HttpClientResponse.matchStatus({
            "400": decodeError("DeleteTask400", DeleteTask400),
            "404": decodeError("DeleteTask404", DeleteTask404),
            "409": decodeError("DeleteTask409", DeleteTask409),
            "500": decodeError("DeleteTask500", DeleteTask500),
            "503": decodeError("DeleteTask503", DeleteTask503),
            "204": () => Effect.void,
            orElse: unexpectedStatus,
          }),
        ),
      ),
    patchTask: (project, id, options) =>
      HttpClientRequest.patch(`/api/projects/${project}/tasks/${id}`).pipe(
        HttpClientRequest.setUrlParams({ branch: options.params?.["branch"] as any }),
        HttpClientRequest.bodyJsonUnsafe(options.payload),
        withResponse(options.config)(
          HttpClientResponse.matchStatus({
            "2xx": decodeSuccess(PatchTask200),
            "400": decodeError("PatchTask400", PatchTask400),
            "404": decodeError("PatchTask404", PatchTask404),
            "409": decodeError("PatchTask409", PatchTask409),
            "500": decodeError("PatchTask500", PatchTask500),
            "503": decodeError("PatchTask503", PatchTask503),
            orElse: unexpectedStatus,
          }),
        ),
      ),
    getTaskBranches: (project, id, options) =>
      HttpClientRequest.get(`/api/projects/${project}/tasks/${id}/branches`).pipe(
        HttpClientRequest.setUrlParams({ fresh: options?.params?.["fresh"] as any }),
        withResponse(options?.config)(
          HttpClientResponse.matchStatus({
            "2xx": decodeSuccess(GetTaskBranches200),
            "400": decodeError("GetTaskBranches400", GetTaskBranches400),
            "404": decodeError("GetTaskBranches404", GetTaskBranches404),
            "500": decodeError("GetTaskBranches500", GetTaskBranches500),
            "503": decodeError("GetTaskBranches503", GetTaskBranches503),
            orElse: unexpectedStatus,
          }),
        ),
      ),
    getTaskTree: (project, id, options) =>
      HttpClientRequest.get(`/api/projects/${project}/tasks/${id}/tree`).pipe(
        HttpClientRequest.setUrlParams({
          branch: options?.params?.["branch"] as any,
          fresh: options?.params?.["fresh"] as any,
          depth: options?.params?.["depth"] as any,
        }),
        withResponse(options?.config)(
          HttpClientResponse.matchStatus({
            "2xx": decodeSuccess(GetTaskTree200),
            "400": decodeError("GetTaskTree400", GetTaskTree400),
            "404": decodeError("GetTaskTree404", GetTaskTree404),
            "500": decodeError("GetTaskTree500", GetTaskTree500),
            "503": decodeError("GetTaskTree503", GetTaskTree503),
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
  readonly getMergedBoard: <Config extends OperationConfig>(
    options: { readonly config?: Config | undefined } | undefined,
  ) => Effect.Effect<
    WithOptionalResponse<typeof GetMergedBoard200.Type, Config>,
    HttpClientError.HttpClientError | SchemaError | TasksClientError<"GetMergedBoard500", typeof GetMergedBoard500.Type>
  >
  readonly listProjects: <Config extends OperationConfig>(
    options: { readonly config?: Config | undefined } | undefined,
  ) => Effect.Effect<
    WithOptionalResponse<typeof ListProjects200.Type, Config>,
    HttpClientError.HttpClientError | SchemaError
  >
  readonly registerProject: <Config extends OperationConfig>(options: {
    readonly payload: typeof RegisterProjectRequestJson.Encoded
    readonly config?: Config | undefined
  }) => Effect.Effect<
    WithOptionalResponse<typeof RegisterProject200.Type | typeof RegisterProject201.Type, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"RegisterProject400", typeof RegisterProject400.Type>
    | TasksClientError<"RegisterProject503", typeof RegisterProject503.Type>
  >
  readonly deleteProject: <Config extends OperationConfig>(
    project: string,
    options: { readonly config?: Config | undefined } | undefined,
  ) => Effect.Effect<
    WithOptionalResponse<void, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"DeleteProject404", typeof DeleteProject404.Type>
    | TasksClientError<"DeleteProject503", typeof DeleteProject503.Type>
  >
  readonly renameProject: <Config extends OperationConfig>(
    project: string,
    options: { readonly payload: typeof RenameProjectRequestJson.Encoded; readonly config?: Config | undefined },
  ) => Effect.Effect<
    WithOptionalResponse<typeof RenameProject200.Type, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"RenameProject400", typeof RenameProject400.Type>
    | TasksClientError<"RenameProject404", typeof RenameProject404.Type>
    | TasksClientError<"RenameProject409", typeof RenameProject409.Type>
    | TasksClientError<"RenameProject503", typeof RenameProject503.Type>
  >
  readonly getBoard: <Config extends OperationConfig>(
    project: string,
    options: { readonly config?: Config | undefined } | undefined,
  ) => Effect.Effect<
    WithOptionalResponse<typeof GetBoard200.Type, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"GetBoard400", typeof GetBoard400.Type>
    | TasksClientError<"GetBoard404", typeof GetBoard404.Type>
    | TasksClientError<"GetBoard500", typeof GetBoard500.Type>
    | TasksClientError<"GetBoard503", typeof GetBoard503.Type>
  >
  readonly getConfig: <Config extends OperationConfig>(
    project: string,
    options: { readonly config?: Config | undefined } | undefined,
  ) => Effect.Effect<
    WithOptionalResponse<typeof GetConfig200.Type, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"GetConfig404", typeof GetConfig404.Type>
    | TasksClientError<"GetConfig503", typeof GetConfig503.Type>
  >
  readonly getMatrix: <Config extends OperationConfig>(
    project: string,
    options:
      | { readonly params?: typeof GetMatrixParams.Encoded | undefined; readonly config?: Config | undefined }
      | undefined,
  ) => Effect.Effect<
    WithOptionalResponse<typeof GetMatrix200.Type, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"GetMatrix404", typeof GetMatrix404.Type>
    | TasksClientError<"GetMatrix500", typeof GetMatrix500.Type>
    | TasksClientError<"GetMatrix503", typeof GetMatrix503.Type>
  >
  readonly listTasks: <Config extends OperationConfig>(
    project: string,
    options:
      | { readonly params?: typeof ListTasksParams.Encoded | undefined; readonly config?: Config | undefined }
      | undefined,
  ) => Effect.Effect<
    WithOptionalResponse<typeof ListTasks200.Type, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"ListTasks400", typeof ListTasks400.Type>
    | TasksClientError<"ListTasks404", typeof ListTasks404.Type>
    | TasksClientError<"ListTasks500", typeof ListTasks500.Type>
    | TasksClientError<"ListTasks503", typeof ListTasks503.Type>
  >
  readonly createTask: <Config extends OperationConfig>(
    project: string,
    options: {
      readonly params?: typeof CreateTaskParams.Encoded | undefined
      readonly payload: typeof CreateTaskRequestJson.Encoded
      readonly config?: Config | undefined
    },
  ) => Effect.Effect<
    WithOptionalResponse<typeof CreateTask201.Type, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"CreateTask400", typeof CreateTask400.Type>
    | TasksClientError<"CreateTask404", typeof CreateTask404.Type>
    | TasksClientError<"CreateTask409", typeof CreateTask409.Type>
    | TasksClientError<"CreateTask500", typeof CreateTask500.Type>
    | TasksClientError<"CreateTask503", typeof CreateTask503.Type>
  >
  readonly getTask: <Config extends OperationConfig>(
    project: string,
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
    | TasksClientError<"GetTask503", typeof GetTask503.Type>
  >
  readonly deleteTask: <Config extends OperationConfig>(
    project: string,
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
    | TasksClientError<"DeleteTask503", typeof DeleteTask503.Type>
  >
  readonly patchTask: <Config extends OperationConfig>(
    project: string,
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
    | TasksClientError<"PatchTask503", typeof PatchTask503.Type>
  >
  readonly getTaskBranches: <Config extends OperationConfig>(
    project: string,
    id: string,
    options:
      | { readonly params?: typeof GetTaskBranchesParams.Encoded | undefined; readonly config?: Config | undefined }
      | undefined,
  ) => Effect.Effect<
    WithOptionalResponse<typeof GetTaskBranches200.Type, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"GetTaskBranches400", typeof GetTaskBranches400.Type>
    | TasksClientError<"GetTaskBranches404", typeof GetTaskBranches404.Type>
    | TasksClientError<"GetTaskBranches500", typeof GetTaskBranches500.Type>
    | TasksClientError<"GetTaskBranches503", typeof GetTaskBranches503.Type>
  >
  readonly getTaskTree: <Config extends OperationConfig>(
    project: string,
    id: string,
    options:
      | { readonly params?: typeof GetTaskTreeParams.Encoded | undefined; readonly config?: Config | undefined }
      | undefined,
  ) => Effect.Effect<
    WithOptionalResponse<typeof GetTaskTree200.Type, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"GetTaskTree400", typeof GetTaskTree400.Type>
    | TasksClientError<"GetTaskTree404", typeof GetTaskTree404.Type>
    | TasksClientError<"GetTaskTree500", typeof GetTaskTree500.Type>
    | TasksClientError<"GetTaskTree503", typeof GetTaskTree503.Type>
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
