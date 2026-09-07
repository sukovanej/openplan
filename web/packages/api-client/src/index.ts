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
export type ChangeKind = "base" | "added" | "modified" | "deleted"
export const ChangeKind = Schema.Literals(["base", "added", "modified", "deleted"])
export type Color =
  | "slate"
  | "red"
  | "orange"
  | "amber"
  | "yellow"
  | "green"
  | "teal"
  | "cyan"
  | "blue"
  | "indigo"
  | "violet"
  | "pink"
export const Color = Schema.Literals([
  "slate",
  "red",
  "orange",
  "amber",
  "yellow",
  "green",
  "teal",
  "cyan",
  "blue",
  "indigo",
  "violet",
  "pink",
])
export type Conflict = { readonly files: ReadonlyArray<string>; readonly worktree: string }
export const Conflict = Schema.Struct({ files: Schema.Array(Schema.String), worktree: Schema.String })
export type CreateComment = { readonly agent?: string; readonly author: string; readonly text: string }
export const CreateComment = Schema.Struct({
  agent: Schema.optionalKey(Schema.String),
  author: Schema.String,
  text: Schema.String,
})
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
export type FlowEdge = { readonly from: string; readonly project: string; readonly to: string }
export const FlowEdge = Schema.Struct({ from: Schema.String, project: Schema.String, to: Schema.String })
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
export type Published = {
  readonly branch: string
  readonly commit: string
  readonly pull_request?: string | null
  readonly remote: string
}
export const Published = Schema.Struct({
  branch: Schema.String,
  commit: Schema.String,
  pull_request: Schema.optionalKey(Schema.Union([Schema.String, Schema.Null])),
  remote: Schema.String,
})
export type Refusal = "tag_referenced" | "tag_unregistered"
export const Refusal = Schema.Literals(["tag_referenced", "tag_unregistered"])
export type RegisterProject = { readonly path: string }
export const RegisterProject = Schema.Struct({ path: Schema.String })
export type RenameProject = { readonly name: string }
export const RenameProject = Schema.Struct({ name: Schema.String })
export type SearchMatch = "key" | "title" | "text"
export const SearchMatch = Schema.Literals(["key", "title", "text"])
export type Status = "backlog" | "todo" | "in_progress" | "in_review" | "done" | "cancelled"
export const Status = Schema.Literals(["backlog", "todo", "in_progress", "in_review", "done", "cancelled"])
export type TaskTreeView = { readonly cycles?: ReadonlyArray<string>; readonly tree: TaskTree }
export const TaskTreeView = Schema.Struct({ cycles: Schema.optionalKey(Schema.Array(Schema.String)), tree: TaskTree })
export type WriteTarget = { readonly branch: string; readonly writable: boolean }
export const WriteTarget = Schema.Struct({ branch: Schema.String, writable: Schema.Boolean })
export type BranchMark = { readonly branch: string; readonly dirty: boolean; readonly kind: ChangeKind }
export const BranchMark = Schema.Struct({ branch: Schema.String, dirty: Schema.Boolean, kind: ChangeKind })
export type CreateTag = { readonly color?: Color; readonly description?: string; readonly name: string }
export const CreateTag = Schema.Struct({
  color: Schema.optionalKey(Color),
  description: Schema.optionalKey(Schema.String),
  name: Schema.String,
})
export type TagPatch = { readonly color?: Color; readonly description?: string | null; readonly name?: string }
export const TagPatch = Schema.Struct({
  color: Schema.optionalKey(Color),
  description: Schema.optionalKey(Schema.Union([Schema.String, Schema.Null])),
  name: Schema.optionalKey(Schema.String),
})
export type TagView = {
  readonly color: Color
  readonly description?: string
  readonly display: string
  readonly name: string
}
export const TagView = Schema.Struct({
  color: Color,
  description: Schema.optionalKey(Schema.String),
  display: Schema.String,
  name: Schema.String,
})
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
export type Field_String = string | FieldError
export const Field_String = Schema.Union([Schema.String, FieldError], { mode: "oneOf" })
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
export type ApiErrorBody = {
  readonly cycles?: ReadonlyArray<ReadonlyArray<string>>
  readonly message: string
  readonly reason?: Refusal
}
export const ApiErrorBody = Schema.Struct({
  cycles: Schema.optionalKey(Schema.Array(Schema.Array(Schema.String))),
  message: Schema.String,
  reason: Schema.optionalKey(Refusal),
})
export type CreateTask = {
  readonly body?: string | null
  readonly dependencies?: ReadonlyArray<string>
  readonly parent?: string | null
  readonly status?: null | Status
  readonly tags?: ReadonlyArray<string>
  readonly title: string
}
export const CreateTask = Schema.Struct({
  body: Schema.optionalKey(Schema.Union([Schema.String, Schema.Null])),
  dependencies: Schema.optionalKey(Schema.Array(Schema.String)),
  parent: Schema.optionalKey(Schema.Union([Schema.String, Schema.Null])),
  status: Schema.optionalKey(Schema.Union([Schema.Null, Status], { mode: "oneOf" })),
  tags: Schema.optionalKey(Schema.Array(Schema.String)),
  title: Schema.String,
})
export type TaskPatch = {
  readonly dependencies?: ReadonlyArray<string>
  readonly parent?: string | null
  readonly rank?: string
  readonly status?: Status
  readonly tags?: ReadonlyArray<string>
}
export const TaskPatch = Schema.Struct({
  dependencies: Schema.optionalKey(Schema.Array(Schema.String)),
  parent: Schema.optionalKey(Schema.Union([Schema.String, Schema.Null])),
  rank: Schema.optionalKey(Schema.String),
  status: Schema.optionalKey(Status),
  tags: Schema.optionalKey(Schema.Array(Schema.String)),
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
export type FlowNode =
  | {
      readonly blocks_count: number
      readonly id: string
      readonly kind: "leaf"
      readonly parent?: string
      readonly position: number
      readonly project: string
      readonly status: Field_Status
      readonly title: string
      readonly wave: number
    }
  | {
      readonly id: string
      readonly kind: "box"
      readonly parent?: string
      readonly project: string
      readonly status: Field_Status
      readonly title: string
    }
  | { readonly id: string; readonly kind: "unresolved"; readonly project: string }
export const FlowNode = Schema.Union(
  [
    Schema.Struct({
      blocks_count: Schema.Number.check(Schema.isInt()).check(Schema.isGreaterThanOrEqualTo(0)),
      id: Schema.String,
      kind: Schema.Literal("leaf"),
      parent: Schema.optionalKey(Schema.String),
      position: Schema.Number.check(Schema.isInt()).check(Schema.isGreaterThanOrEqualTo(0)),
      project: Schema.String,
      status: Field_Status,
      title: Schema.String,
      wave: Schema.Number.check(Schema.isInt()).check(Schema.isGreaterThanOrEqualTo(0)),
    }),
    Schema.Struct({
      id: Schema.String,
      kind: Schema.Literal("box"),
      parent: Schema.optionalKey(Schema.String),
      project: Schema.String,
      status: Field_Status,
      title: Schema.String,
    }),
    Schema.Struct({ id: Schema.String, kind: Schema.Literal("unresolved"), project: Schema.String }),
  ],
  { mode: "oneOf" },
)
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
export type Comment = {
  readonly agent?: string | null
  readonly at: Field_Rfc3339
  readonly author: Field_String
  readonly text: string
}
export const Comment = Schema.Struct({
  agent: Schema.optionalKey(Schema.Union([Schema.String, Schema.Null])),
  at: Field_Rfc3339,
  author: Field_String,
  text: Schema.String,
})
export type FrontmatterFields = {
  readonly created: Field_Rfc3339
  readonly dependencies: Field_Vec_String
  readonly parent: Field_Option_String
  readonly rank: Field_Option_String
  readonly status: Field_Status
  readonly tags: Field_Vec_String
}
export const FrontmatterFields = Schema.Struct({
  created: Field_Rfc3339,
  dependencies: Field_Vec_String,
  parent: Field_Option_String,
  rank: Field_Option_String,
  status: Field_Status,
  tags: Field_Vec_String,
})
export type Flow = { readonly edges: ReadonlyArray<FlowEdge>; readonly nodes: ReadonlyArray<FlowNode> }
export const Flow = Schema.Struct({ edges: Schema.Array(FlowEdge), nodes: Schema.Array(FlowNode) })
export type BranchComments = { readonly branch: string; readonly comments: ReadonlyArray<Comment> }
export const BranchComments = Schema.Struct({ branch: Schema.String, comments: Schema.Array(Comment) })
export type Metadata = { readonly kind: MetadataErrorTag; readonly message: string } | FrontmatterFields
export const Metadata = Schema.Union(
  [Schema.Struct({ kind: MetadataErrorTag, message: Schema.String }), FrontmatterFields],
  { mode: "oneOf" },
)
export type TaskDetail = {
  readonly blocks?: ReadonlyArray<TaskRef>
  readonly body: string
  readonly branches: ReadonlyArray<BranchState>
  readonly children?: ReadonlyArray<TaskChild>
  readonly comments?: ReadonlyArray<Comment>
  readonly depends_on?: ReadonlyArray<TaskRef>
  readonly headline: string
  readonly id: string
  readonly metadata: Metadata
  readonly parent_title?: string
  readonly project: string
  readonly refs?: ReadonlyArray<TaskRef>
  readonly title: string
  readonly updated: Field_Rfc3339
  readonly write_target?: WriteTarget
}
export const TaskDetail = Schema.Struct({
  blocks: Schema.optionalKey(Schema.Array(TaskRef)),
  body: Schema.String,
  branches: Schema.Array(BranchState),
  children: Schema.optionalKey(Schema.Array(TaskChild)),
  comments: Schema.optionalKey(Schema.Array(Comment)),
  depends_on: Schema.optionalKey(Schema.Array(TaskRef)),
  headline: Schema.String,
  id: Schema.String,
  metadata: Metadata,
  parent_title: Schema.optionalKey(Schema.String),
  project: Schema.String,
  refs: Schema.optionalKey(Schema.Array(TaskRef)),
  title: Schema.String,
  updated: Field_Rfc3339,
  write_target: Schema.optionalKey(WriteTarget),
})
export type TaskListItem = {
  readonly branches: ReadonlyArray<BranchState>
  readonly comment_count: number
  readonly headline: string
  readonly id: string
  readonly metadata: Metadata
  readonly project: string
  readonly title: string
  readonly updated: Field_Rfc3339
  readonly write_target?: WriteTarget
}
export const TaskListItem = Schema.Struct({
  branches: Schema.Array(BranchState),
  comment_count: Schema.Number.check(Schema.isInt()).check(Schema.isGreaterThanOrEqualTo(0)),
  headline: Schema.String,
  id: Schema.String,
  metadata: Metadata,
  project: Schema.String,
  title: Schema.String,
  updated: Field_Rfc3339,
  write_target: Schema.optionalKey(WriteTarget),
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
export type SearchHit = { readonly branch: string; readonly matched: SearchMatch; readonly task: TaskListItem }
export const SearchHit = Schema.Struct({ branch: Schema.String, matched: SearchMatch, task: TaskListItem })
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
export type RollingUpdates = { readonly conflict?: null | Conflict; readonly pending: ReadonlyArray<MatrixCell> }
export const RollingUpdates = Schema.Struct({
  conflict: Schema.optionalKey(Schema.Union([Schema.Null, Conflict], { mode: "oneOf" })),
  pending: Schema.Array(MatrixCell),
})
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
export type GetFlowParams = {
  readonly project?: ReadonlyArray<string>
  readonly status?: ReadonlyArray<Status>
  readonly task?: ReadonlyArray<string>
  readonly tag?: ReadonlyArray<string>
}
export const GetFlowParams = Schema.Struct({
  project: Schema.optionalKey(Schema.Array(Schema.String)),
  status: Schema.optionalKey(Schema.Array(Status)),
  task: Schema.optionalKey(Schema.Array(Schema.String)),
  tag: Schema.optionalKey(Schema.Array(Schema.String)),
})
export type GetFlow200 = Flow
export const GetFlow200 = Flow
export type GetFlow400 = ApiErrorBody
export const GetFlow400 = ApiErrorBody
export type GetFlow404 = ApiErrorBody
export const GetFlow404 = ApiErrorBody
export type GetFlow422 = ApiErrorBody
export const GetFlow422 = ApiErrorBody
export type GetFlow500 = ApiErrorBody
export const GetFlow500 = ApiErrorBody
export type GetFlow503 = ApiErrorBody
export const GetFlow503 = ApiErrorBody
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
export type GetRollingUpdates200 = RollingUpdates
export const GetRollingUpdates200 = RollingUpdates
export type GetRollingUpdates404 = ApiErrorBody
export const GetRollingUpdates404 = ApiErrorBody
export type GetRollingUpdates500 = ApiErrorBody
export const GetRollingUpdates500 = ApiErrorBody
export type GetRollingUpdates503 = ApiErrorBody
export const GetRollingUpdates503 = ApiErrorBody
export type PublishRollingUpdates200 = Published
export const PublishRollingUpdates200 = Published
export type PublishRollingUpdates404 = ApiErrorBody
export const PublishRollingUpdates404 = ApiErrorBody
export type PublishRollingUpdates409 = ApiErrorBody
export const PublishRollingUpdates409 = ApiErrorBody
export type PublishRollingUpdates503 = ApiErrorBody
export const PublishRollingUpdates503 = ApiErrorBody
export type SearchProjectParams = { readonly q?: string; readonly fresh?: boolean }
export const SearchProjectParams = Schema.Struct({
  q: Schema.optionalKey(Schema.String),
  fresh: Schema.optionalKey(Schema.Boolean),
})
export type SearchProject200 = ReadonlyArray<SearchHit>
export const SearchProject200 = Schema.Array(SearchHit)
export type SearchProject404 = ApiErrorBody
export const SearchProject404 = ApiErrorBody
export type SearchProject500 = ApiErrorBody
export const SearchProject500 = ApiErrorBody
export type SearchProject503 = ApiErrorBody
export const SearchProject503 = ApiErrorBody
export type ListTagsParams = { readonly branch?: string }
export const ListTagsParams = Schema.Struct({ branch: Schema.optionalKey(Schema.String) })
export type ListTags200 = ReadonlyArray<TagView>
export const ListTags200 = Schema.Array(TagView)
export type ListTags400 = ApiErrorBody
export const ListTags400 = ApiErrorBody
export type ListTags404 = ApiErrorBody
export const ListTags404 = ApiErrorBody
export type ListTags409 = ApiErrorBody
export const ListTags409 = ApiErrorBody
export type ListTags422 = ApiErrorBody
export const ListTags422 = ApiErrorBody
export type ListTags500 = ApiErrorBody
export const ListTags500 = ApiErrorBody
export type ListTags503 = ApiErrorBody
export const ListTags503 = ApiErrorBody
export type CreateTagParams = { readonly branch?: string }
export const CreateTagParams = Schema.Struct({ branch: Schema.optionalKey(Schema.String) })
export type CreateTagRequestJson = CreateTag
export const CreateTagRequestJson = CreateTag
export type CreateTag201 = TagView
export const CreateTag201 = TagView
export type CreateTag400 = ApiErrorBody
export const CreateTag400 = ApiErrorBody
export type CreateTag404 = ApiErrorBody
export const CreateTag404 = ApiErrorBody
export type CreateTag409 = ApiErrorBody
export const CreateTag409 = ApiErrorBody
export type CreateTag422 = ApiErrorBody
export const CreateTag422 = ApiErrorBody
export type CreateTag500 = ApiErrorBody
export const CreateTag500 = ApiErrorBody
export type CreateTag503 = ApiErrorBody
export const CreateTag503 = ApiErrorBody
export type GetTagParams = { readonly branch?: string }
export const GetTagParams = Schema.Struct({ branch: Schema.optionalKey(Schema.String) })
export type GetTag200 = TagView
export const GetTag200 = TagView
export type GetTag400 = ApiErrorBody
export const GetTag400 = ApiErrorBody
export type GetTag404 = ApiErrorBody
export const GetTag404 = ApiErrorBody
export type GetTag409 = ApiErrorBody
export const GetTag409 = ApiErrorBody
export type GetTag422 = ApiErrorBody
export const GetTag422 = ApiErrorBody
export type GetTag503 = ApiErrorBody
export const GetTag503 = ApiErrorBody
export type DeleteTagParams = { readonly branch?: string; readonly force?: boolean }
export const DeleteTagParams = Schema.Struct({
  branch: Schema.optionalKey(Schema.String),
  force: Schema.optionalKey(Schema.Boolean),
})
export type DeleteTag400 = ApiErrorBody
export const DeleteTag400 = ApiErrorBody
export type DeleteTag404 = ApiErrorBody
export const DeleteTag404 = ApiErrorBody
export type DeleteTag409 = ApiErrorBody
export const DeleteTag409 = ApiErrorBody
export type DeleteTag500 = ApiErrorBody
export const DeleteTag500 = ApiErrorBody
export type DeleteTag503 = ApiErrorBody
export const DeleteTag503 = ApiErrorBody
export type PatchTagParams = { readonly branch?: string }
export const PatchTagParams = Schema.Struct({ branch: Schema.optionalKey(Schema.String) })
export type PatchTagRequestJson = TagPatch
export const PatchTagRequestJson = TagPatch
export type PatchTag200 = TagView
export const PatchTag200 = TagView
export type PatchTag400 = ApiErrorBody
export const PatchTag400 = ApiErrorBody
export type PatchTag404 = ApiErrorBody
export const PatchTag404 = ApiErrorBody
export type PatchTag409 = ApiErrorBody
export const PatchTag409 = ApiErrorBody
export type PatchTag422 = ApiErrorBody
export const PatchTag422 = ApiErrorBody
export type PatchTag500 = ApiErrorBody
export const PatchTag500 = ApiErrorBody
export type PatchTag503 = ApiErrorBody
export const PatchTag503 = ApiErrorBody
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
export type ListCommentsParams = { readonly branch?: string | null; readonly fresh?: boolean }
export const ListCommentsParams = Schema.Struct({
  branch: Schema.optionalKey(Schema.Union([Schema.String, Schema.Null])),
  fresh: Schema.optionalKey(Schema.Boolean),
})
export type ListComments200 = ReadonlyArray<Comment>
export const ListComments200 = Schema.Array(Comment)
export type ListComments400 = ApiErrorBody
export const ListComments400 = ApiErrorBody
export type ListComments404 = ApiErrorBody
export const ListComments404 = ApiErrorBody
export type ListComments500 = ApiErrorBody
export const ListComments500 = ApiErrorBody
export type ListComments503 = ApiErrorBody
export const ListComments503 = ApiErrorBody
export type AddCommentParams = { readonly branch?: string }
export const AddCommentParams = Schema.Struct({ branch: Schema.optionalKey(Schema.String) })
export type AddCommentRequestJson = CreateComment
export const AddCommentRequestJson = CreateComment
export type AddComment201 = Comment
export const AddComment201 = Comment
export type AddComment400 = ApiErrorBody
export const AddComment400 = ApiErrorBody
export type AddComment404 = ApiErrorBody
export const AddComment404 = ApiErrorBody
export type AddComment409 = ApiErrorBody
export const AddComment409 = ApiErrorBody
export type AddComment500 = ApiErrorBody
export const AddComment500 = ApiErrorBody
export type AddComment503 = ApiErrorBody
export const AddComment503 = ApiErrorBody
export type ListBranchCommentsParams = { readonly fresh?: boolean }
export const ListBranchCommentsParams = Schema.Struct({ fresh: Schema.optionalKey(Schema.Boolean) })
export type ListBranchComments200 = ReadonlyArray<BranchComments>
export const ListBranchComments200 = Schema.Array(BranchComments)
export type ListBranchComments400 = ApiErrorBody
export const ListBranchComments400 = ApiErrorBody
export type ListBranchComments404 = ApiErrorBody
export const ListBranchComments404 = ApiErrorBody
export type ListBranchComments500 = ApiErrorBody
export const ListBranchComments500 = ApiErrorBody
export type ListBranchComments503 = ApiErrorBody
export const ListBranchComments503 = ApiErrorBody
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
export type SearchAllParams = { readonly q?: string; readonly fresh?: boolean }
export const SearchAllParams = Schema.Struct({
  q: Schema.optionalKey(Schema.String),
  fresh: Schema.optionalKey(Schema.Boolean),
})
export type SearchAll200 = ReadonlyArray<SearchHit>
export const SearchAll200 = Schema.Array(SearchHit)
export type SearchAll500 = ApiErrorBody
export const SearchAll500 = ApiErrorBody
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
    getFlow: (options) =>
      HttpClientRequest.get(`/api/flow`).pipe(
        HttpClientRequest.setUrlParams({
          project: options?.params?.["project"] as any,
          status: options?.params?.["status"] as any,
          task: options?.params?.["task"] as any,
          tag: options?.params?.["tag"] as any,
        }),
        withResponse(options?.config)(
          HttpClientResponse.matchStatus({
            "2xx": decodeSuccess(GetFlow200),
            "400": decodeError("GetFlow400", GetFlow400),
            "404": decodeError("GetFlow404", GetFlow404),
            "422": decodeError("GetFlow422", GetFlow422),
            "500": decodeError("GetFlow500", GetFlow500),
            "503": decodeError("GetFlow503", GetFlow503),
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
    getRollingUpdates: (project, options) =>
      HttpClientRequest.get(`/api/projects/${project}/rolling-updates`).pipe(
        withResponse(options?.config)(
          HttpClientResponse.matchStatus({
            "2xx": decodeSuccess(GetRollingUpdates200),
            "404": decodeError("GetRollingUpdates404", GetRollingUpdates404),
            "500": decodeError("GetRollingUpdates500", GetRollingUpdates500),
            "503": decodeError("GetRollingUpdates503", GetRollingUpdates503),
            orElse: unexpectedStatus,
          }),
        ),
      ),
    publishRollingUpdates: (project, options) =>
      HttpClientRequest.post(`/api/projects/${project}/rolling-updates/publish`).pipe(
        withResponse(options?.config)(
          HttpClientResponse.matchStatus({
            "2xx": decodeSuccess(PublishRollingUpdates200),
            "404": decodeError("PublishRollingUpdates404", PublishRollingUpdates404),
            "409": decodeError("PublishRollingUpdates409", PublishRollingUpdates409),
            "503": decodeError("PublishRollingUpdates503", PublishRollingUpdates503),
            orElse: unexpectedStatus,
          }),
        ),
      ),
    searchProject: (project, options) =>
      HttpClientRequest.get(`/api/projects/${project}/search`).pipe(
        HttpClientRequest.setUrlParams({ q: options?.params?.["q"] as any, fresh: options?.params?.["fresh"] as any }),
        withResponse(options?.config)(
          HttpClientResponse.matchStatus({
            "2xx": decodeSuccess(SearchProject200),
            "404": decodeError("SearchProject404", SearchProject404),
            "500": decodeError("SearchProject500", SearchProject500),
            "503": decodeError("SearchProject503", SearchProject503),
            orElse: unexpectedStatus,
          }),
        ),
      ),
    listTags: (project, options) =>
      HttpClientRequest.get(`/api/projects/${project}/tags`).pipe(
        HttpClientRequest.setUrlParams({ branch: options?.params?.["branch"] as any }),
        withResponse(options?.config)(
          HttpClientResponse.matchStatus({
            "2xx": decodeSuccess(ListTags200),
            "400": decodeError("ListTags400", ListTags400),
            "404": decodeError("ListTags404", ListTags404),
            "409": decodeError("ListTags409", ListTags409),
            "422": decodeError("ListTags422", ListTags422),
            "500": decodeError("ListTags500", ListTags500),
            "503": decodeError("ListTags503", ListTags503),
            orElse: unexpectedStatus,
          }),
        ),
      ),
    createTag: (project, options) =>
      HttpClientRequest.post(`/api/projects/${project}/tags`).pipe(
        HttpClientRequest.setUrlParams({ branch: options.params?.["branch"] as any }),
        HttpClientRequest.bodyJsonUnsafe(options.payload),
        withResponse(options.config)(
          HttpClientResponse.matchStatus({
            "2xx": decodeSuccess(CreateTag201),
            "400": decodeError("CreateTag400", CreateTag400),
            "404": decodeError("CreateTag404", CreateTag404),
            "409": decodeError("CreateTag409", CreateTag409),
            "422": decodeError("CreateTag422", CreateTag422),
            "500": decodeError("CreateTag500", CreateTag500),
            "503": decodeError("CreateTag503", CreateTag503),
            orElse: unexpectedStatus,
          }),
        ),
      ),
    getTag: (project, name, options) =>
      HttpClientRequest.get(`/api/projects/${project}/tags/${name}`).pipe(
        HttpClientRequest.setUrlParams({ branch: options?.params?.["branch"] as any }),
        withResponse(options?.config)(
          HttpClientResponse.matchStatus({
            "2xx": decodeSuccess(GetTag200),
            "400": decodeError("GetTag400", GetTag400),
            "404": decodeError("GetTag404", GetTag404),
            "409": decodeError("GetTag409", GetTag409),
            "422": decodeError("GetTag422", GetTag422),
            "503": decodeError("GetTag503", GetTag503),
            orElse: unexpectedStatus,
          }),
        ),
      ),
    deleteTag: (project, name, options) =>
      HttpClientRequest.delete(`/api/projects/${project}/tags/${name}`).pipe(
        HttpClientRequest.setUrlParams({
          branch: options?.params?.["branch"] as any,
          force: options?.params?.["force"] as any,
        }),
        withResponse(options?.config)(
          HttpClientResponse.matchStatus({
            "400": decodeError("DeleteTag400", DeleteTag400),
            "404": decodeError("DeleteTag404", DeleteTag404),
            "409": decodeError("DeleteTag409", DeleteTag409),
            "500": decodeError("DeleteTag500", DeleteTag500),
            "503": decodeError("DeleteTag503", DeleteTag503),
            "204": () => Effect.void,
            orElse: unexpectedStatus,
          }),
        ),
      ),
    patchTag: (project, name, options) =>
      HttpClientRequest.patch(`/api/projects/${project}/tags/${name}`).pipe(
        HttpClientRequest.setUrlParams({ branch: options.params?.["branch"] as any }),
        HttpClientRequest.bodyJsonUnsafe(options.payload),
        withResponse(options.config)(
          HttpClientResponse.matchStatus({
            "2xx": decodeSuccess(PatchTag200),
            "400": decodeError("PatchTag400", PatchTag400),
            "404": decodeError("PatchTag404", PatchTag404),
            "409": decodeError("PatchTag409", PatchTag409),
            "422": decodeError("PatchTag422", PatchTag422),
            "500": decodeError("PatchTag500", PatchTag500),
            "503": decodeError("PatchTag503", PatchTag503),
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
    listComments: (project, id, options) =>
      HttpClientRequest.get(`/api/projects/${project}/tasks/${id}/comments`).pipe(
        HttpClientRequest.setUrlParams({
          branch: options?.params?.["branch"] as any,
          fresh: options?.params?.["fresh"] as any,
        }),
        withResponse(options?.config)(
          HttpClientResponse.matchStatus({
            "2xx": decodeSuccess(ListComments200),
            "400": decodeError("ListComments400", ListComments400),
            "404": decodeError("ListComments404", ListComments404),
            "500": decodeError("ListComments500", ListComments500),
            "503": decodeError("ListComments503", ListComments503),
            orElse: unexpectedStatus,
          }),
        ),
      ),
    addComment: (project, id, options) =>
      HttpClientRequest.post(`/api/projects/${project}/tasks/${id}/comments`).pipe(
        HttpClientRequest.setUrlParams({ branch: options.params?.["branch"] as any }),
        HttpClientRequest.bodyJsonUnsafe(options.payload),
        withResponse(options.config)(
          HttpClientResponse.matchStatus({
            "2xx": decodeSuccess(AddComment201),
            "400": decodeError("AddComment400", AddComment400),
            "404": decodeError("AddComment404", AddComment404),
            "409": decodeError("AddComment409", AddComment409),
            "500": decodeError("AddComment500", AddComment500),
            "503": decodeError("AddComment503", AddComment503),
            orElse: unexpectedStatus,
          }),
        ),
      ),
    listBranchComments: (project, id, options) =>
      HttpClientRequest.get(`/api/projects/${project}/tasks/${id}/comments/branches`).pipe(
        HttpClientRequest.setUrlParams({ fresh: options?.params?.["fresh"] as any }),
        withResponse(options?.config)(
          HttpClientResponse.matchStatus({
            "2xx": decodeSuccess(ListBranchComments200),
            "400": decodeError("ListBranchComments400", ListBranchComments400),
            "404": decodeError("ListBranchComments404", ListBranchComments404),
            "500": decodeError("ListBranchComments500", ListBranchComments500),
            "503": decodeError("ListBranchComments503", ListBranchComments503),
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
    searchAll: (options) =>
      HttpClientRequest.get(`/api/search`).pipe(
        HttpClientRequest.setUrlParams({ q: options?.params?.["q"] as any, fresh: options?.params?.["fresh"] as any }),
        withResponse(options?.config)(
          HttpClientResponse.matchStatus({
            "2xx": decodeSuccess(SearchAll200),
            "500": decodeError("SearchAll500", SearchAll500),
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
  readonly getFlow: <Config extends OperationConfig>(
    options:
      | { readonly params?: typeof GetFlowParams.Encoded | undefined; readonly config?: Config | undefined }
      | undefined,
  ) => Effect.Effect<
    WithOptionalResponse<typeof GetFlow200.Type, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"GetFlow400", typeof GetFlow400.Type>
    | TasksClientError<"GetFlow404", typeof GetFlow404.Type>
    | TasksClientError<"GetFlow422", typeof GetFlow422.Type>
    | TasksClientError<"GetFlow500", typeof GetFlow500.Type>
    | TasksClientError<"GetFlow503", typeof GetFlow503.Type>
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
  readonly getRollingUpdates: <Config extends OperationConfig>(
    project: string,
    options: { readonly config?: Config | undefined } | undefined,
  ) => Effect.Effect<
    WithOptionalResponse<typeof GetRollingUpdates200.Type, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"GetRollingUpdates404", typeof GetRollingUpdates404.Type>
    | TasksClientError<"GetRollingUpdates500", typeof GetRollingUpdates500.Type>
    | TasksClientError<"GetRollingUpdates503", typeof GetRollingUpdates503.Type>
  >
  readonly publishRollingUpdates: <Config extends OperationConfig>(
    project: string,
    options: { readonly config?: Config | undefined } | undefined,
  ) => Effect.Effect<
    WithOptionalResponse<typeof PublishRollingUpdates200.Type, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"PublishRollingUpdates404", typeof PublishRollingUpdates404.Type>
    | TasksClientError<"PublishRollingUpdates409", typeof PublishRollingUpdates409.Type>
    | TasksClientError<"PublishRollingUpdates503", typeof PublishRollingUpdates503.Type>
  >
  readonly searchProject: <Config extends OperationConfig>(
    project: string,
    options:
      | { readonly params?: typeof SearchProjectParams.Encoded | undefined; readonly config?: Config | undefined }
      | undefined,
  ) => Effect.Effect<
    WithOptionalResponse<typeof SearchProject200.Type, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"SearchProject404", typeof SearchProject404.Type>
    | TasksClientError<"SearchProject500", typeof SearchProject500.Type>
    | TasksClientError<"SearchProject503", typeof SearchProject503.Type>
  >
  readonly listTags: <Config extends OperationConfig>(
    project: string,
    options:
      | { readonly params?: typeof ListTagsParams.Encoded | undefined; readonly config?: Config | undefined }
      | undefined,
  ) => Effect.Effect<
    WithOptionalResponse<typeof ListTags200.Type, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"ListTags400", typeof ListTags400.Type>
    | TasksClientError<"ListTags404", typeof ListTags404.Type>
    | TasksClientError<"ListTags409", typeof ListTags409.Type>
    | TasksClientError<"ListTags422", typeof ListTags422.Type>
    | TasksClientError<"ListTags500", typeof ListTags500.Type>
    | TasksClientError<"ListTags503", typeof ListTags503.Type>
  >
  readonly createTag: <Config extends OperationConfig>(
    project: string,
    options: {
      readonly params?: typeof CreateTagParams.Encoded | undefined
      readonly payload: typeof CreateTagRequestJson.Encoded
      readonly config?: Config | undefined
    },
  ) => Effect.Effect<
    WithOptionalResponse<typeof CreateTag201.Type, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"CreateTag400", typeof CreateTag400.Type>
    | TasksClientError<"CreateTag404", typeof CreateTag404.Type>
    | TasksClientError<"CreateTag409", typeof CreateTag409.Type>
    | TasksClientError<"CreateTag422", typeof CreateTag422.Type>
    | TasksClientError<"CreateTag500", typeof CreateTag500.Type>
    | TasksClientError<"CreateTag503", typeof CreateTag503.Type>
  >
  readonly getTag: <Config extends OperationConfig>(
    project: string,
    name: string,
    options:
      | { readonly params?: typeof GetTagParams.Encoded | undefined; readonly config?: Config | undefined }
      | undefined,
  ) => Effect.Effect<
    WithOptionalResponse<typeof GetTag200.Type, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"GetTag400", typeof GetTag400.Type>
    | TasksClientError<"GetTag404", typeof GetTag404.Type>
    | TasksClientError<"GetTag409", typeof GetTag409.Type>
    | TasksClientError<"GetTag422", typeof GetTag422.Type>
    | TasksClientError<"GetTag503", typeof GetTag503.Type>
  >
  readonly deleteTag: <Config extends OperationConfig>(
    project: string,
    name: string,
    options:
      | { readonly params?: typeof DeleteTagParams.Encoded | undefined; readonly config?: Config | undefined }
      | undefined,
  ) => Effect.Effect<
    WithOptionalResponse<void, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"DeleteTag400", typeof DeleteTag400.Type>
    | TasksClientError<"DeleteTag404", typeof DeleteTag404.Type>
    | TasksClientError<"DeleteTag409", typeof DeleteTag409.Type>
    | TasksClientError<"DeleteTag500", typeof DeleteTag500.Type>
    | TasksClientError<"DeleteTag503", typeof DeleteTag503.Type>
  >
  readonly patchTag: <Config extends OperationConfig>(
    project: string,
    name: string,
    options: {
      readonly params?: typeof PatchTagParams.Encoded | undefined
      readonly payload: typeof PatchTagRequestJson.Encoded
      readonly config?: Config | undefined
    },
  ) => Effect.Effect<
    WithOptionalResponse<typeof PatchTag200.Type, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"PatchTag400", typeof PatchTag400.Type>
    | TasksClientError<"PatchTag404", typeof PatchTag404.Type>
    | TasksClientError<"PatchTag409", typeof PatchTag409.Type>
    | TasksClientError<"PatchTag422", typeof PatchTag422.Type>
    | TasksClientError<"PatchTag500", typeof PatchTag500.Type>
    | TasksClientError<"PatchTag503", typeof PatchTag503.Type>
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
  readonly listComments: <Config extends OperationConfig>(
    project: string,
    id: string,
    options:
      | { readonly params?: typeof ListCommentsParams.Encoded | undefined; readonly config?: Config | undefined }
      | undefined,
  ) => Effect.Effect<
    WithOptionalResponse<typeof ListComments200.Type, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"ListComments400", typeof ListComments400.Type>
    | TasksClientError<"ListComments404", typeof ListComments404.Type>
    | TasksClientError<"ListComments500", typeof ListComments500.Type>
    | TasksClientError<"ListComments503", typeof ListComments503.Type>
  >
  readonly addComment: <Config extends OperationConfig>(
    project: string,
    id: string,
    options: {
      readonly params?: typeof AddCommentParams.Encoded | undefined
      readonly payload: typeof AddCommentRequestJson.Encoded
      readonly config?: Config | undefined
    },
  ) => Effect.Effect<
    WithOptionalResponse<typeof AddComment201.Type, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"AddComment400", typeof AddComment400.Type>
    | TasksClientError<"AddComment404", typeof AddComment404.Type>
    | TasksClientError<"AddComment409", typeof AddComment409.Type>
    | TasksClientError<"AddComment500", typeof AddComment500.Type>
    | TasksClientError<"AddComment503", typeof AddComment503.Type>
  >
  readonly listBranchComments: <Config extends OperationConfig>(
    project: string,
    id: string,
    options:
      | { readonly params?: typeof ListBranchCommentsParams.Encoded | undefined; readonly config?: Config | undefined }
      | undefined,
  ) => Effect.Effect<
    WithOptionalResponse<typeof ListBranchComments200.Type, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"ListBranchComments400", typeof ListBranchComments400.Type>
    | TasksClientError<"ListBranchComments404", typeof ListBranchComments404.Type>
    | TasksClientError<"ListBranchComments500", typeof ListBranchComments500.Type>
    | TasksClientError<"ListBranchComments503", typeof ListBranchComments503.Type>
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
  readonly searchAll: <Config extends OperationConfig>(
    options:
      | { readonly params?: typeof SearchAllParams.Encoded | undefined; readonly config?: Config | undefined }
      | undefined,
  ) => Effect.Effect<
    WithOptionalResponse<typeof SearchAll200.Type, Config>,
    HttpClientError.HttpClientError | SchemaError | TasksClientError<"SearchAll500", typeof SearchAll500.Type>
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
