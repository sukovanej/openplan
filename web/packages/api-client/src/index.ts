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
export type MetadataErrorTag = "error"
export const MetadataErrorTag = Schema.Literal("error").annotate({ identifier: "MetadataErrorTag" })
export type FieldError = { readonly kind: "missing" } | { readonly kind: "invalid"; readonly message: string }
export const FieldError = Schema.Union(
  [
    Schema.Struct({ kind: Schema.Literal("missing") }),
    Schema.Struct({ kind: Schema.Literal("invalid"), message: Schema.String }),
  ],
  { mode: "oneOf" },
).annotate({ identifier: "FieldError" })
export type ConflictTag = "conflict"
export const ConflictTag = Schema.Literal("conflict").annotate({ identifier: "ConflictTag" })
export type ConflictSide_Rfc3339 = { readonly label: string; readonly value: string }
export const ConflictSide_Rfc3339 = Schema.Struct({
  label: Schema.String,
  value: Schema.String.annotate({ format: "date-time" }),
}).annotate({ identifier: "ConflictSide_Rfc3339" })
export type ConflictSide_Vec = { readonly label: string; readonly value: ReadonlyArray<string> }
export const ConflictSide_Vec = Schema.Struct({ label: Schema.String, value: Schema.Array(Schema.String) }).annotate({
  identifier: "ConflictSide_Vec",
})
export type ConflictSide_Option = { readonly label: string; readonly value: null | string }
export const ConflictSide_Option = Schema.Struct({
  label: Schema.String,
  value: Schema.Union([Schema.Null, Schema.String], { mode: "oneOf" }),
}).annotate({ identifier: "ConflictSide_Option" })
export type ConflictSide_Status = {
  readonly label: string
  readonly value: "backlog" | "todo" | "in_progress" | "in_review" | "done" | "cancelled"
}
export const ConflictSide_Status = Schema.Struct({
  label: Schema.String,
  value: Schema.Literals(["backlog", "todo", "in_progress", "in_review", "done", "cancelled"]),
}).annotate({ identifier: "ConflictSide_Status" })
export type ProblemCode =
  | "field"
  | "title"
  | "comment"
  | "reference"
  | "tag"
  | "parent_cycle"
  | "dependency_cycle"
  | "duplicate_number"
export const ProblemCode = Schema.Literals([
  "field",
  "title",
  "comment",
  "reference",
  "tag",
  "parent_cycle",
  "dependency_cycle",
  "duplicate_number",
]).annotate({ identifier: "ProblemCode" })
export type Status = "backlog" | "todo" | "in_progress" | "in_review" | "done" | "cancelled"
export const Status = Schema.Literals(["backlog", "todo", "in_progress", "in_review", "done", "cancelled"]).annotate({
  identifier: "Status",
})
export type DiagramSource = { readonly source: string }
export const DiagramSource = Schema.Struct({ source: Schema.String }).annotate({ identifier: "DiagramSource" })
export type Drawing = { readonly height: number; readonly svg: string; readonly width: number }
export const Drawing = Schema.Struct({
  height: Schema.Number.annotate({ format: "float" }).check(
    Schema.isFinite().annotate({ expected: "a finite number" }),
  ),
  svg: Schema.String,
  width: Schema.Number.annotate({ format: "float" }).check(Schema.isFinite().annotate({ expected: "a finite number" })),
}).annotate({ identifier: "Drawing" })
export type SourcePosition = { readonly column: number; readonly line: number }
export const SourcePosition = Schema.Struct({
  column: Schema.Number.check(Schema.isInt().annotate({ expected: "an integer" })).check(
    Schema.isGreaterThanOrEqualTo(0).annotate({ expected: "a value greater than or equal to 0" }),
  ),
  line: Schema.Number.check(Schema.isInt().annotate({ expected: "an integer" })).check(
    Schema.isGreaterThanOrEqualTo(0).annotate({ expected: "a value greater than or equal to 0" }),
  ),
}).annotate({ identifier: "SourcePosition" })
export type Refusal = "tag_referenced" | "tag_unregistered"
export const Refusal = Schema.Literals(["tag_referenced", "tag_unregistered"]).annotate({ identifier: "Refusal" })
export type FlowEdge = { readonly from: string; readonly project: string; readonly to: string }
export const FlowEdge = Schema.Struct({ from: Schema.String, project: Schema.String, to: Schema.String }).annotate({
  identifier: "FlowEdge",
})
export type BackendKind = "git" | "local"
export const BackendKind = Schema.Literals(["git", "local"]).annotate({ identifier: "BackendKind" })
export type ProjectStatus = { readonly state: "ok" } | { readonly reason: string; readonly state: "error" }
export const ProjectStatus = Schema.Union(
  [
    Schema.Struct({ state: Schema.Literal("ok") }),
    Schema.Struct({ reason: Schema.String, state: Schema.Literal("error") }),
  ],
  { mode: "oneOf" },
).annotate({ identifier: "ProjectStatus" })
export type Rfc3339 = string
export const Rfc3339 = Schema.String.annotate({ format: "date-time", identifier: "Rfc3339" })
export type RenameProject = { readonly name: string }
export const RenameProject = Schema.Struct({ name: Schema.String }).annotate({ identifier: "RenameProject" })
export type CreateSession = { readonly agent?: string | null; readonly prompt: string; readonly task?: string | null }
export const CreateSession = Schema.Struct({
  agent: Schema.optionalKey(Schema.Union([Schema.String, Schema.Null]).annotate({ examples: ["claude_code"] })),
  prompt: Schema.String,
  task: Schema.optionalKey(Schema.Union([Schema.String, Schema.Null])),
}).annotate({ identifier: "CreateSession" })
export type CreatedSession = { readonly id: string }
export const CreatedSession = Schema.Struct({ id: Schema.String }).annotate({ identifier: "CreatedSession" })
export type Decision = { readonly [x: string]: Schema.Json }
export const Decision = Schema.Record(Schema.String, Schema.Json.annotate({ expected: "JSON value" })).annotate({
  identifier: "Decision",
})
export type Say = { readonly text: string }
export const Say = Schema.Struct({ text: Schema.String }).annotate({ identifier: "Say" })
export type DocumentChangeKind = "added" | "modified" | "removed"
export const DocumentChangeKind = Schema.Literals(["added", "modified", "removed"]).annotate({
  identifier: "DocumentChangeKind",
})
export type SearchMatch = "key" | "title" | "text"
export const SearchMatch = Schema.Literals(["key", "title", "text"]).annotate({ identifier: "SearchMatch" })
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
]).annotate({ identifier: "Color" })
export type CreatedTask = { readonly id: string }
export const CreatedTask = Schema.Struct({ id: Schema.String }).annotate({ identifier: "CreatedTask" })
export type ConflictSide_String = { readonly label: string; readonly value: string }
export const ConflictSide_String = Schema.Struct({ label: Schema.String, value: Schema.String }).annotate({
  identifier: "ConflictSide_String",
})
export type CreateComment = { readonly agent?: string; readonly author: string; readonly text: string }
export const CreateComment = Schema.Struct({
  agent: Schema.optionalKey(Schema.String),
  author: Schema.String,
  text: Schema.String,
}).annotate({ identifier: "CreateComment" })
export type WriteTaskFile = { readonly text: string }
export const WriteTaskFile = Schema.Struct({ text: Schema.String }).annotate({ identifier: "WriteTaskFile" })
export type ResolveConflict = { readonly block: string; readonly text: string }
export const ResolveConflict = Schema.Struct({ block: Schema.String, text: Schema.String }).annotate({
  identifier: "ResolveConflict",
})
export type TaskTreeView = { readonly cycles?: ReadonlyArray<string>; readonly tree: TaskTree }
export const TaskTreeView = Schema.Struct({
  cycles: Schema.optionalKey(Schema.Array(Schema.String)),
  tree: TaskTree,
}).annotate({ identifier: "TaskTreeView" })
export type DaemonInfo = {
  readonly pid: number
  readonly port: number
  readonly started_at: number
  readonly version: string
}
export const DaemonInfo = Schema.Struct({
  pid: Schema.Number.annotate({ format: "int32" })
    .check(Schema.isInt().annotate({ expected: "an integer" }))
    .check(Schema.isGreaterThanOrEqualTo(0).annotate({ expected: "a value greater than or equal to 0" })),
  port: Schema.Number.annotate({ format: "int32" })
    .check(Schema.isInt().annotate({ expected: "an integer" }))
    .check(Schema.isGreaterThanOrEqualTo(0).annotate({ expected: "a value greater than or equal to 0" })),
  started_at: Schema.Number.annotate({ format: "int64" })
    .check(Schema.isInt().annotate({ expected: "an integer" }))
    .check(Schema.isGreaterThanOrEqualTo(0).annotate({ expected: "a value greater than or equal to 0" })),
  version: Schema.String,
}).annotate({ identifier: "DaemonInfo" })
export type FieldConflict_Rfc3339 = {
  readonly kind: ConflictTag
  readonly sides: ReadonlyArray<ConflictSide_Rfc3339>
  readonly value: string
}
export const FieldConflict_Rfc3339 = Schema.Struct({
  kind: ConflictTag,
  sides: Schema.Array(ConflictSide_Rfc3339),
  value: Schema.String.annotate({ format: "date-time" }),
}).annotate({ identifier: "FieldConflict_Rfc3339" })
export type FieldConflict_Vec = {
  readonly kind: ConflictTag
  readonly sides: ReadonlyArray<ConflictSide_Vec>
  readonly value: ReadonlyArray<string>
}
export const FieldConflict_Vec = Schema.Struct({
  kind: ConflictTag,
  sides: Schema.Array(ConflictSide_Vec),
  value: Schema.Array(Schema.String),
}).annotate({ identifier: "FieldConflict_Vec" })
export type FieldConflict_Option = {
  readonly kind: ConflictTag
  readonly sides: ReadonlyArray<ConflictSide_Option>
  readonly value: null | string
}
export const FieldConflict_Option = Schema.Struct({
  kind: ConflictTag,
  sides: Schema.Array(ConflictSide_Option),
  value: Schema.Union([Schema.Null, Schema.String], { mode: "oneOf" }),
}).annotate({ identifier: "FieldConflict_Option" })
export type FieldConflict_Status = {
  readonly kind: ConflictTag
  readonly sides: ReadonlyArray<ConflictSide_Status>
  readonly value: "backlog" | "todo" | "in_progress" | "in_review" | "done" | "cancelled"
}
export const FieldConflict_Status = Schema.Struct({
  kind: ConflictTag,
  sides: Schema.Array(ConflictSide_Status),
  value: Schema.Literals(["backlog", "todo", "in_progress", "in_review", "done", "cancelled"]),
}).annotate({ identifier: "FieldConflict_Status" })
export type Problem = { readonly code: ProblemCode; readonly message: string }
export const Problem = Schema.Struct({ code: ProblemCode, message: Schema.String }).annotate({ identifier: "Problem" })
export type FieldChange =
  | { readonly field: "number"; readonly from: string; readonly to: string }
  | { readonly field: "status"; readonly from: Status; readonly to: Status }
  | { readonly field: "parent"; readonly from?: string; readonly to?: string }
  | { readonly field: "order" }
  | { readonly field: "dependencies"; readonly from: ReadonlyArray<string>; readonly to: ReadonlyArray<string> }
  | { readonly field: "tags"; readonly from: ReadonlyArray<string>; readonly to: ReadonlyArray<string> }
  | { readonly field: "title"; readonly from?: string; readonly to?: string }
  | { readonly field: "description" }
  | { readonly added: number; readonly field: "comments"; readonly removed: number }
  | { readonly field: "conflicts"; readonly from: number; readonly to: number }
  | { readonly field: "other"; readonly name: string }
  | { readonly field: "frontmatter" }
export const FieldChange = Schema.Union(
  [
    Schema.Struct({ field: Schema.Literal("number"), from: Schema.String, to: Schema.String }),
    Schema.Struct({ field: Schema.Literal("status"), from: Status, to: Status }),
    Schema.Struct({
      field: Schema.Literal("parent"),
      from: Schema.optionalKey(Schema.String),
      to: Schema.optionalKey(Schema.String),
    }),
    Schema.Struct({ field: Schema.Literal("order") }),
    Schema.Struct({
      field: Schema.Literal("dependencies"),
      from: Schema.Array(Schema.String),
      to: Schema.Array(Schema.String),
    }),
    Schema.Struct({
      field: Schema.Literal("tags"),
      from: Schema.Array(Schema.String),
      to: Schema.Array(Schema.String),
    }),
    Schema.Struct({
      field: Schema.Literal("title"),
      from: Schema.optionalKey(Schema.String),
      to: Schema.optionalKey(Schema.String),
    }),
    Schema.Struct({ field: Schema.Literal("description") }),
    Schema.Struct({
      added: Schema.Number.check(Schema.isInt().annotate({ expected: "an integer" })).check(
        Schema.isGreaterThanOrEqualTo(0).annotate({ expected: "a value greater than or equal to 0" }),
      ),
      field: Schema.Literal("comments"),
      removed: Schema.Number.check(Schema.isInt().annotate({ expected: "an integer" })).check(
        Schema.isGreaterThanOrEqualTo(0).annotate({ expected: "a value greater than or equal to 0" }),
      ),
    }),
    Schema.Struct({
      field: Schema.Literal("conflicts"),
      from: Schema.Number.check(Schema.isInt().annotate({ expected: "an integer" })).check(
        Schema.isGreaterThanOrEqualTo(0).annotate({ expected: "a value greater than or equal to 0" }),
      ),
      to: Schema.Number.check(Schema.isInt().annotate({ expected: "an integer" })).check(
        Schema.isGreaterThanOrEqualTo(0).annotate({ expected: "a value greater than or equal to 0" }),
      ),
    }),
    Schema.Struct({ field: Schema.Literal("other"), name: Schema.String }),
    Schema.Struct({ field: Schema.Literal("frontmatter") }),
  ],
  { mode: "oneOf" },
).annotate({ identifier: "FieldChange" })
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
}).annotate({ identifier: "CreateTask" })
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
}).annotate({ identifier: "TaskPatch" })
export type ApiErrorBody = {
  readonly cycles?: ReadonlyArray<ReadonlyArray<string>>
  readonly message: string
  readonly position?: SourcePosition
  readonly reason?: Refusal
}
export const ApiErrorBody = Schema.Struct({
  cycles: Schema.optionalKey(Schema.Array(Schema.Array(Schema.String))),
  message: Schema.String,
  position: Schema.optionalKey(SourcePosition),
  reason: Schema.optionalKey(Refusal),
}).annotate({ identifier: "ApiErrorBody" })
export type RegisterProject = { readonly abbreviation?: string; readonly backend?: BackendKind; readonly path: string }
export const RegisterProject = Schema.Struct({
  abbreviation: Schema.optionalKey(Schema.String),
  backend: Schema.optionalKey(BackendKind),
  path: Schema.String,
}).annotate({ identifier: "RegisterProject" })
export type SyncView = {
  readonly ahead: number
  readonly behind: number
  readonly error?: string
  readonly last_attempt?: Rfc3339
  readonly last_success?: Rfc3339
  readonly remote: string
  readonly syncing: boolean
}
export const SyncView = Schema.Struct({
  ahead: Schema.Number.check(Schema.isInt().annotate({ expected: "an integer" })).check(
    Schema.isGreaterThanOrEqualTo(0).annotate({ expected: "a value greater than or equal to 0" }),
  ),
  behind: Schema.Number.check(Schema.isInt().annotate({ expected: "an integer" })).check(
    Schema.isGreaterThanOrEqualTo(0).annotate({ expected: "a value greater than or equal to 0" }),
  ),
  error: Schema.optionalKey(Schema.String),
  last_attempt: Schema.optionalKey(Rfc3339),
  last_success: Schema.optionalKey(Rfc3339),
  remote: Schema.String,
  syncing: Schema.Boolean,
}).annotate({ identifier: "SyncView" })
export type SessionSummary = {
  readonly agent: string
  readonly id: string
  readonly started_at: Rfc3339
  readonly status: { readonly [x: string]: Schema.Json }
  readonly task?: string | null
}
export const SessionSummary = Schema.Struct({
  agent: Schema.String.annotate({ examples: ["claude_code"] }),
  id: Schema.String,
  started_at: Rfc3339,
  status: Schema.Record(Schema.String, Schema.Json.annotate({ expected: "JSON value" })),
  task: Schema.optionalKey(Schema.Union([Schema.String, Schema.Null])),
}).annotate({ identifier: "SessionSummary" })
export type RevisionView = {
  readonly agent?: string
  readonly at: Rfc3339
  readonly author: string
  readonly email?: string
  readonly id: string
  readonly message: string
  readonly parents: ReadonlyArray<string>
}
export const RevisionView = Schema.Struct({
  agent: Schema.optionalKey(Schema.String),
  at: Rfc3339,
  author: Schema.String,
  email: Schema.optionalKey(Schema.String),
  id: Schema.String,
  message: Schema.String,
  parents: Schema.Array(Schema.String),
}).annotate({ identifier: "RevisionView" })
export type DocumentChange = {
  readonly kind: DocumentChangeKind
  readonly path: string
  readonly tag?: string
  readonly task?: string
}
export const DocumentChange = Schema.Struct({
  kind: DocumentChangeKind,
  path: Schema.String,
  tag: Schema.optionalKey(Schema.String),
  task: Schema.optionalKey(Schema.String),
}).annotate({ identifier: "DocumentChange" })
export type TagChange = { readonly kind: DocumentChangeKind; readonly renamed_from?: string; readonly tag: string }
export const TagChange = Schema.Struct({
  kind: DocumentChangeKind,
  renamed_from: Schema.optionalKey(Schema.String),
  tag: Schema.String,
}).annotate({ identifier: "TagChange" })
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
}).annotate({ identifier: "TagView" })
export type CreateTag = { readonly color?: Color; readonly description?: string; readonly name: string }
export const CreateTag = Schema.Struct({
  color: Schema.optionalKey(Color),
  description: Schema.optionalKey(Schema.String),
  name: Schema.String,
}).annotate({ identifier: "CreateTag" })
export type TagPatch = { readonly color?: Color; readonly description?: string | null; readonly name?: string }
export const TagPatch = Schema.Struct({
  color: Schema.optionalKey(Color),
  description: Schema.optionalKey(Schema.Union([Schema.String, Schema.Null])),
  name: Schema.optionalKey(Schema.String),
}).annotate({ identifier: "TagPatch" })
export type FieldConflict_String = {
  readonly kind: ConflictTag
  readonly sides: ReadonlyArray<ConflictSide_String>
  readonly value: string
}
export const FieldConflict_String = Schema.Struct({
  kind: ConflictTag,
  sides: Schema.Array(ConflictSide_String),
  value: Schema.String,
}).annotate({ identifier: "FieldConflict_String" })
export type Field_Rfc3339 = string | FieldError | FieldConflict_Rfc3339
export const Field_Rfc3339 = Schema.Union(
  [Schema.String.annotate({ format: "date-time" }), FieldError, FieldConflict_Rfc3339],
  { mode: "oneOf" },
).annotate({ identifier: "Field_Rfc3339" })
export type Field_Vec_String = ReadonlyArray<string> | FieldError | FieldConflict_Vec
export const Field_Vec_String = Schema.Union([Schema.Array(Schema.String), FieldError, FieldConflict_Vec], {
  mode: "oneOf",
}).annotate({ identifier: "Field_Vec_String" })
export type Field_Option_String = null | string | FieldError | FieldConflict_Option
export const Field_Option_String = Schema.Union(
  [Schema.Union([Schema.Null, Schema.String], { mode: "oneOf" }), FieldError, FieldConflict_Option],
  { mode: "oneOf" },
).annotate({ identifier: "Field_Option_String" })
export type Field_Status =
  | "backlog"
  | "todo"
  | "in_progress"
  | "in_review"
  | "done"
  | "cancelled"
  | FieldError
  | FieldConflict_Status
export const Field_Status = Schema.Union(
  [
    Schema.Literals(["backlog", "todo", "in_progress", "in_review", "done", "cancelled"]),
    FieldError,
    FieldConflict_Status,
  ],
  { mode: "oneOf" },
).annotate({ identifier: "Field_Status" })
export type TaskChange = {
  readonly fields?: ReadonlyArray<FieldChange>
  readonly kind: DocumentChangeKind
  readonly task: string
  readonly title?: string
}
export const TaskChange = Schema.Struct({
  fields: Schema.optionalKey(Schema.Array(FieldChange)),
  kind: DocumentChangeKind,
  task: Schema.String,
  title: Schema.optionalKey(Schema.String),
}).annotate({ identifier: "TaskChange" })
export type ProjectView = {
  readonly abbreviation: string
  readonly backend: BackendKind
  readonly git_common_dir?: string
  readonly name: string
  readonly root: string
  readonly status: ProjectStatus
  readonly sync?: SyncView
}
export const ProjectView = Schema.Struct({
  abbreviation: Schema.String,
  backend: BackendKind,
  git_common_dir: Schema.optionalKey(Schema.String),
  name: Schema.String,
  root: Schema.String,
  status: ProjectStatus,
  sync: Schema.optionalKey(SyncView),
}).annotate({ identifier: "ProjectView" })
export type SyncResult = {
  readonly merged: boolean
  readonly received: number
  readonly sent: number
  readonly status: SyncView
}
export const SyncResult = Schema.Struct({
  merged: Schema.Boolean,
  received: Schema.Number.check(Schema.isInt().annotate({ expected: "an integer" })).check(
    Schema.isGreaterThanOrEqualTo(0).annotate({ expected: "a value greater than or equal to 0" }),
  ),
  sent: Schema.Number.check(Schema.isInt().annotate({ expected: "an integer" })).check(
    Schema.isGreaterThanOrEqualTo(0).annotate({ expected: "a value greater than or equal to 0" }),
  ),
  status: SyncView,
}).annotate({ identifier: "SyncResult" })
export type Field_String = string | FieldError | FieldConflict_String
export const Field_String = Schema.Union([Schema.String, FieldError, FieldConflict_String], { mode: "oneOf" }).annotate(
  { identifier: "Field_String" },
)
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
}).annotate({ identifier: "FrontmatterFields" })
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
      blocks_count: Schema.Number.check(Schema.isInt().annotate({ expected: "an integer" })).check(
        Schema.isGreaterThanOrEqualTo(0).annotate({ expected: "a value greater than or equal to 0" }),
      ),
      id: Schema.String,
      kind: Schema.Literal("leaf"),
      parent: Schema.optionalKey(Schema.String),
      position: Schema.Number.check(Schema.isInt().annotate({ expected: "an integer" })).check(
        Schema.isGreaterThanOrEqualTo(0).annotate({ expected: "a value greater than or equal to 0" }),
      ),
      project: Schema.String,
      status: Field_Status,
      title: Schema.String,
      wave: Schema.Number.check(Schema.isInt().annotate({ expected: "an integer" })).check(
        Schema.isGreaterThanOrEqualTo(0).annotate({ expected: "a value greater than or equal to 0" }),
      ),
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
).annotate({ identifier: "FlowNode" })
export type TaskRef = { readonly id: string; readonly status: Field_Status; readonly title: string }
export const TaskRef = Schema.Struct({ id: Schema.String, status: Field_Status, title: Schema.String }).annotate({
  identifier: "TaskRef",
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
}).annotate({ identifier: "TaskChild" })
export type HistoryEntry = {
  readonly changes: ReadonlyArray<DocumentChange>
  readonly revision: RevisionView
  readonly summary: ReadonlyArray<string>
  readonly tags: ReadonlyArray<TagChange>
  readonly tasks: ReadonlyArray<TaskChange>
}
export const HistoryEntry = Schema.Struct({
  changes: Schema.Array(DocumentChange),
  revision: RevisionView,
  summary: Schema.Array(Schema.String),
  tags: Schema.Array(TagChange),
  tasks: Schema.Array(TaskChange),
}).annotate({ identifier: "HistoryEntry" })
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
}).annotate({ identifier: "Comment" })
export type Metadata = { readonly kind: MetadataErrorTag; readonly message: string } | FrontmatterFields
export const Metadata = Schema.Union(
  [Schema.Struct({ kind: MetadataErrorTag, message: Schema.String }), FrontmatterFields],
  { mode: "oneOf" },
).annotate({ identifier: "Metadata" })
export type Flow = { readonly edges: ReadonlyArray<FlowEdge>; readonly nodes: ReadonlyArray<FlowNode> }
export const Flow = Schema.Struct({ edges: Schema.Array(FlowEdge), nodes: Schema.Array(FlowNode) }).annotate({
  identifier: "Flow",
})
export type TaskListItem = {
  readonly comment_count: number
  readonly conflicts: number
  readonly id: string
  readonly metadata: Metadata
  readonly problems: ReadonlyArray<Problem>
  readonly project: string
  readonly title: string
  readonly updated: Field_Rfc3339
}
export const TaskListItem = Schema.Struct({
  comment_count: Schema.Number.check(Schema.isInt().annotate({ expected: "an integer" })).check(
    Schema.isGreaterThanOrEqualTo(0).annotate({ expected: "a value greater than or equal to 0" }),
  ),
  conflicts: Schema.Number.check(Schema.isInt().annotate({ expected: "an integer" })).check(
    Schema.isGreaterThanOrEqualTo(0).annotate({ expected: "a value greater than or equal to 0" }),
  ),
  id: Schema.String,
  metadata: Metadata,
  problems: Schema.Array(Problem),
  project: Schema.String,
  title: Schema.String,
  updated: Field_Rfc3339,
}).annotate({ identifier: "TaskListItem" })
export type TaskDetail = {
  readonly blocks?: ReadonlyArray<TaskRef>
  readonly body: string
  readonly children?: ReadonlyArray<TaskChild>
  readonly comments?: ReadonlyArray<Comment>
  readonly conflicts: number
  readonly depends_on?: ReadonlyArray<TaskRef>
  readonly id: string
  readonly metadata: Metadata
  readonly parent_title?: string
  readonly problems: ReadonlyArray<Problem>
  readonly project: string
  readonly refs?: ReadonlyArray<TaskRef>
  readonly title: string
  readonly updated: Field_Rfc3339
}
export const TaskDetail = Schema.Struct({
  blocks: Schema.optionalKey(Schema.Array(TaskRef)),
  body: Schema.String,
  children: Schema.optionalKey(Schema.Array(TaskChild)),
  comments: Schema.optionalKey(Schema.Array(Comment)),
  conflicts: Schema.Number.check(Schema.isInt().annotate({ expected: "an integer" })).check(
    Schema.isGreaterThanOrEqualTo(0).annotate({ expected: "a value greater than or equal to 0" }),
  ),
  depends_on: Schema.optionalKey(Schema.Array(TaskRef)),
  id: Schema.String,
  metadata: Metadata,
  parent_title: Schema.optionalKey(Schema.String),
  problems: Schema.Array(Problem),
  project: Schema.String,
  refs: Schema.optionalKey(Schema.Array(TaskRef)),
  title: Schema.String,
  updated: Field_Rfc3339,
}).annotate({ identifier: "TaskDetail" })
export type TaskSnapshot = {
  readonly body: string
  readonly comments?: ReadonlyArray<Comment>
  readonly metadata: Metadata
  readonly raw: string
  readonly title: string
}
export const TaskSnapshot = Schema.Struct({
  body: Schema.String,
  comments: Schema.optionalKey(Schema.Array(Comment)),
  metadata: Metadata,
  raw: Schema.String,
  title: Schema.String,
}).annotate({ identifier: "TaskSnapshot" })
export type BoardRow = {
  readonly depth: number
  readonly has_children: boolean
  readonly parent_title?: string
  readonly task: TaskListItem
}
export const BoardRow = Schema.Struct({
  depth: Schema.Number.check(Schema.isInt().annotate({ expected: "an integer" })).check(
    Schema.isGreaterThanOrEqualTo(0).annotate({ expected: "a value greater than or equal to 0" }),
  ),
  has_children: Schema.Boolean,
  parent_title: Schema.optionalKey(Schema.String),
  task: TaskListItem,
}).annotate({ identifier: "BoardRow" })
export type SearchHit = { readonly matched: SearchMatch; readonly task: TaskListItem }
export const SearchHit = Schema.Struct({ matched: SearchMatch, task: TaskListItem }).annotate({
  identifier: "SearchHit",
})
export type TaskAtRevision = { readonly id: string; readonly revision: string; readonly task?: TaskSnapshot }
export const TaskAtRevision = Schema.Struct({
  id: Schema.String,
  revision: Schema.String,
  task: Schema.optionalKey(TaskSnapshot),
}).annotate({ identifier: "TaskAtRevision" })
export type BoardGroup = { readonly rows: ReadonlyArray<BoardRow>; readonly status?: Status }
export const BoardGroup = Schema.Struct({ rows: Schema.Array(BoardRow), status: Schema.optionalKey(Status) }).annotate({
  identifier: "BoardGroup",
})
export type Board = { readonly groups: ReadonlyArray<BoardGroup> }
export const Board = Schema.Struct({ groups: Schema.Array(BoardGroup) }).annotate({ identifier: "Board" })
// recursive definitions
const __recursive_TaskTree = Schema.Struct({
  children: Schema.Array(Schema.suspend((): Schema.Codec<TaskTree> => TaskTree)),
  id: Schema.String,
  metadata: Metadata,
  title: Schema.String,
}).annotate({ identifier: "TaskTree" })
// schemas
export type GetMergedBoard200 = Board
export const GetMergedBoard200 = Board
export type DrawDiagramRequestJson = DiagramSource
export const DrawDiagramRequestJson = DiagramSource
export type DrawDiagram200 = Drawing
export const DrawDiagram200 = Drawing
export type DrawDiagram422 = ApiErrorBody
export const DrawDiagram422 = ApiErrorBody
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
export type GetFlow503 = ApiErrorBody
export const GetFlow503 = ApiErrorBody
export type DrawFlowParams = {
  readonly project?: ReadonlyArray<string>
  readonly status?: ReadonlyArray<Status>
  readonly task?: ReadonlyArray<string>
  readonly tag?: ReadonlyArray<string>
  readonly width?: number
  readonly height?: number
}
export const DrawFlowParams = Schema.Struct({
  project: Schema.optionalKey(Schema.Array(Schema.String)),
  status: Schema.optionalKey(Schema.Array(Status)),
  task: Schema.optionalKey(Schema.Array(Schema.String)),
  tag: Schema.optionalKey(Schema.Array(Schema.String)),
  width: Schema.optionalKey(
    Schema.Number.annotate({ format: "int32" })
      .check(Schema.isInt().annotate({ expected: "an integer" }))
      .check(Schema.isGreaterThanOrEqualTo(0).annotate({ expected: "a value greater than or equal to 0" })),
  ),
  height: Schema.optionalKey(
    Schema.Number.annotate({ format: "int32" })
      .check(Schema.isInt().annotate({ expected: "an integer" }))
      .check(Schema.isGreaterThanOrEqualTo(0).annotate({ expected: "a value greater than or equal to 0" })),
  ),
})
export type DrawFlow200 = Drawing
export const DrawFlow200 = Drawing
export type DrawFlow400 = ApiErrorBody
export const DrawFlow400 = ApiErrorBody
export type DrawFlow404 = ApiErrorBody
export const DrawFlow404 = ApiErrorBody
export type DrawFlow422 = ApiErrorBody
export const DrawFlow422 = ApiErrorBody
export type DrawFlow503 = ApiErrorBody
export const DrawFlow503 = ApiErrorBody
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
export type RegisterProject409 = ApiErrorBody
export const RegisterProject409 = ApiErrorBody
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
export type ListSessions200 = ReadonlyArray<SessionSummary>
export const ListSessions200 = Schema.Array(SessionSummary)
export type ListSessions404 = ApiErrorBody
export const ListSessions404 = ApiErrorBody
export type ListSessions503 = ApiErrorBody
export const ListSessions503 = ApiErrorBody
export type CreateSessionRequestJson = CreateSession
export const CreateSessionRequestJson = CreateSession
export type CreateSession201 = CreatedSession
export const CreateSession201 = CreatedSession
export type CreateSession400 = ApiErrorBody
export const CreateSession400 = ApiErrorBody
export type CreateSession404 = ApiErrorBody
export const CreateSession404 = ApiErrorBody
export type CreateSession503 = ApiErrorBody
export const CreateSession503 = ApiErrorBody
export type DeleteSession404 = ApiErrorBody
export const DeleteSession404 = ApiErrorBody
export type DeleteSession503 = ApiErrorBody
export const DeleteSession503 = ApiErrorBody
export type ApproveRequestJson = Decision
export const ApproveRequestJson = Decision
export type Approve404 = ApiErrorBody
export const Approve404 = ApiErrorBody
export type Approve503 = ApiErrorBody
export const Approve503 = ApiErrorBody
export type InterruptSession404 = ApiErrorBody
export const InterruptSession404 = ApiErrorBody
export type InterruptSession503 = ApiErrorBody
export const InterruptSession503 = ApiErrorBody
export type PromptSessionRequestJson = Say
export const PromptSessionRequestJson = Say
export type PromptSession404 = ApiErrorBody
export const PromptSession404 = ApiErrorBody
export type PromptSession409 = ApiErrorBody
export const PromptSession409 = ApiErrorBody
export type PromptSession503 = ApiErrorBody
export const PromptSession503 = ApiErrorBody
export type GetBoard200 = Board
export const GetBoard200 = Board
export type GetBoard404 = ApiErrorBody
export const GetBoard404 = ApiErrorBody
export type GetBoard503 = ApiErrorBody
export const GetBoard503 = ApiErrorBody
export type ProjectHistoryParams = { readonly before?: string | null; readonly limit?: number | null }
export const ProjectHistoryParams = Schema.Struct({
  before: Schema.optionalKey(Schema.Union([Schema.String, Schema.Null])),
  limit: Schema.optionalKey(
    Schema.Union([
      Schema.Number.check(Schema.isInt().annotate({ expected: "an integer" })).check(
        Schema.isGreaterThanOrEqualTo(0).annotate({ expected: "a value greater than or equal to 0" }),
      ),
      Schema.Null,
    ]),
  ),
})
export type ProjectHistory200 = ReadonlyArray<HistoryEntry>
export const ProjectHistory200 = Schema.Array(HistoryEntry)
export type ProjectHistory404 = ApiErrorBody
export const ProjectHistory404 = ApiErrorBody
export type ProjectHistory503 = ApiErrorBody
export const ProjectHistory503 = ApiErrorBody
export type SearchProjectParams = { readonly q?: string; readonly fresh?: boolean }
export const SearchProjectParams = Schema.Struct({
  q: Schema.optionalKey(Schema.String),
  fresh: Schema.optionalKey(Schema.Boolean),
})
export type SearchProject200 = ReadonlyArray<SearchHit>
export const SearchProject200 = Schema.Array(SearchHit)
export type SearchProject404 = ApiErrorBody
export const SearchProject404 = ApiErrorBody
export type SearchProject503 = ApiErrorBody
export const SearchProject503 = ApiErrorBody
export type GetSync200 = SyncView
export const GetSync200 = SyncView
export type GetSync404 = ApiErrorBody
export const GetSync404 = ApiErrorBody
export type GetSync503 = ApiErrorBody
export const GetSync503 = ApiErrorBody
export type RunSync200 = SyncResult
export const RunSync200 = SyncResult
export type RunSync404 = ApiErrorBody
export const RunSync404 = ApiErrorBody
export type RunSync502 = ApiErrorBody
export const RunSync502 = ApiErrorBody
export type RunSync503 = ApiErrorBody
export const RunSync503 = ApiErrorBody
export type ListTags200 = ReadonlyArray<TagView>
export const ListTags200 = Schema.Array(TagView)
export type ListTags404 = ApiErrorBody
export const ListTags404 = ApiErrorBody
export type ListTags422 = ApiErrorBody
export const ListTags422 = ApiErrorBody
export type ListTags503 = ApiErrorBody
export const ListTags503 = ApiErrorBody
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
export type CreateTag503 = ApiErrorBody
export const CreateTag503 = ApiErrorBody
export type GetTag200 = TagView
export const GetTag200 = TagView
export type GetTag400 = ApiErrorBody
export const GetTag400 = ApiErrorBody
export type GetTag404 = ApiErrorBody
export const GetTag404 = ApiErrorBody
export type GetTag422 = ApiErrorBody
export const GetTag422 = ApiErrorBody
export type GetTag503 = ApiErrorBody
export const GetTag503 = ApiErrorBody
export type DeleteTagParams = { readonly force?: boolean }
export const DeleteTagParams = Schema.Struct({ force: Schema.optionalKey(Schema.Boolean) })
export type DeleteTag400 = ApiErrorBody
export const DeleteTag400 = ApiErrorBody
export type DeleteTag404 = ApiErrorBody
export const DeleteTag404 = ApiErrorBody
export type DeleteTag409 = ApiErrorBody
export const DeleteTag409 = ApiErrorBody
export type DeleteTag503 = ApiErrorBody
export const DeleteTag503 = ApiErrorBody
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
export type PatchTag503 = ApiErrorBody
export const PatchTag503 = ApiErrorBody
export type ListTasksParams = { readonly fresh?: boolean }
export const ListTasksParams = Schema.Struct({ fresh: Schema.optionalKey(Schema.Boolean) })
export type ListTasks200 = ReadonlyArray<TaskListItem>
export const ListTasks200 = Schema.Array(TaskListItem)
export type ListTasks404 = ApiErrorBody
export const ListTasks404 = ApiErrorBody
export type ListTasks503 = ApiErrorBody
export const ListTasks503 = ApiErrorBody
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
export type CreateTask503 = ApiErrorBody
export const CreateTask503 = ApiErrorBody
export type GetTaskParams = { readonly fresh?: boolean }
export const GetTaskParams = Schema.Struct({ fresh: Schema.optionalKey(Schema.Boolean) })
export type GetTask200 = TaskDetail
export const GetTask200 = TaskDetail
export type GetTask400 = ApiErrorBody
export const GetTask400 = ApiErrorBody
export type GetTask404 = ApiErrorBody
export const GetTask404 = ApiErrorBody
export type GetTask503 = ApiErrorBody
export const GetTask503 = ApiErrorBody
export type DeleteTask400 = ApiErrorBody
export const DeleteTask400 = ApiErrorBody
export type DeleteTask404 = ApiErrorBody
export const DeleteTask404 = ApiErrorBody
export type DeleteTask503 = ApiErrorBody
export const DeleteTask503 = ApiErrorBody
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
export type PatchTask503 = ApiErrorBody
export const PatchTask503 = ApiErrorBody
export type ListCommentsParams = { readonly fresh?: boolean }
export const ListCommentsParams = Schema.Struct({ fresh: Schema.optionalKey(Schema.Boolean) })
export type ListComments200 = ReadonlyArray<Comment>
export const ListComments200 = Schema.Array(Comment)
export type ListComments400 = ApiErrorBody
export const ListComments400 = ApiErrorBody
export type ListComments404 = ApiErrorBody
export const ListComments404 = ApiErrorBody
export type ListComments503 = ApiErrorBody
export const ListComments503 = ApiErrorBody
export type AddCommentRequestJson = CreateComment
export const AddCommentRequestJson = CreateComment
export type AddComment201 = Comment
export const AddComment201 = Comment
export type AddComment400 = ApiErrorBody
export const AddComment400 = ApiErrorBody
export type AddComment404 = ApiErrorBody
export const AddComment404 = ApiErrorBody
export type AddComment503 = ApiErrorBody
export const AddComment503 = ApiErrorBody
export type WriteTaskFileRequestJson = WriteTaskFile
export const WriteTaskFileRequestJson = WriteTaskFile
export type WriteTaskFile200 = TaskDetail
export const WriteTaskFile200 = TaskDetail
export type WriteTaskFile400 = ApiErrorBody
export const WriteTaskFile400 = ApiErrorBody
export type WriteTaskFile404 = ApiErrorBody
export const WriteTaskFile404 = ApiErrorBody
export type WriteTaskFile409 = ApiErrorBody
export const WriteTaskFile409 = ApiErrorBody
export type WriteTaskFile503 = ApiErrorBody
export const WriteTaskFile503 = ApiErrorBody
export type TaskHistoryParams = { readonly before?: string | null; readonly limit?: number | null }
export const TaskHistoryParams = Schema.Struct({
  before: Schema.optionalKey(Schema.Union([Schema.String, Schema.Null])),
  limit: Schema.optionalKey(
    Schema.Union([
      Schema.Number.check(Schema.isInt().annotate({ expected: "an integer" })).check(
        Schema.isGreaterThanOrEqualTo(0).annotate({ expected: "a value greater than or equal to 0" }),
      ),
      Schema.Null,
    ]),
  ),
})
export type TaskHistory200 = ReadonlyArray<HistoryEntry>
export const TaskHistory200 = Schema.Array(HistoryEntry)
export type TaskHistory400 = ApiErrorBody
export const TaskHistory400 = ApiErrorBody
export type TaskHistory404 = ApiErrorBody
export const TaskHistory404 = ApiErrorBody
export type TaskHistory503 = ApiErrorBody
export const TaskHistory503 = ApiErrorBody
export type ResolveConflictRequestJson = ResolveConflict
export const ResolveConflictRequestJson = ResolveConflict
export type ResolveConflict200 = TaskDetail
export const ResolveConflict200 = TaskDetail
export type ResolveConflict400 = ApiErrorBody
export const ResolveConflict400 = ApiErrorBody
export type ResolveConflict404 = ApiErrorBody
export const ResolveConflict404 = ApiErrorBody
export type ResolveConflict409 = ApiErrorBody
export const ResolveConflict409 = ApiErrorBody
export type ResolveConflict503 = ApiErrorBody
export const ResolveConflict503 = ApiErrorBody
export type TaskRevision200 = TaskAtRevision
export const TaskRevision200 = TaskAtRevision
export type TaskRevision400 = ApiErrorBody
export const TaskRevision400 = ApiErrorBody
export type TaskRevision404 = ApiErrorBody
export const TaskRevision404 = ApiErrorBody
export type TaskRevision503 = ApiErrorBody
export const TaskRevision503 = ApiErrorBody
export type GetTaskTreeParams = { readonly fresh?: boolean; readonly depth?: number | null }
export const GetTaskTreeParams = Schema.Struct({
  fresh: Schema.optionalKey(Schema.Boolean),
  depth: Schema.optionalKey(
    Schema.Union([
      Schema.Number.check(Schema.isInt().annotate({ expected: "an integer" })).check(
        Schema.isGreaterThanOrEqualTo(0).annotate({ expected: "a value greater than or equal to 0" }),
      ),
      Schema.Null,
    ]),
  ),
})
export type GetTaskTree200 = TaskTreeView
export const GetTaskTree200 = TaskTreeView
export type GetTaskTree400 = ApiErrorBody
export const GetTaskTree400 = ApiErrorBody
export type GetTaskTree404 = ApiErrorBody
export const GetTaskTree404 = ApiErrorBody
export type GetTaskTree503 = ApiErrorBody
export const GetTaskTree503 = ApiErrorBody
export type SearchAllParams = { readonly q?: string; readonly fresh?: boolean }
export const SearchAllParams = Schema.Struct({
  q: Schema.optionalKey(Schema.String),
  fresh: Schema.optionalKey(Schema.Boolean),
})
export type SearchAll200 = ReadonlyArray<SearchHit>
export const SearchAll200 = Schema.Array(SearchHit)
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
  const __encodePathParam = encodeURIComponent
  const __makePathRequest = (
    method: (url: string) => HttpClientRequest.HttpClientRequest,
    parameters: ReadonlyArray<string>,
    getPath: () => string,
  ) =>
    Effect.suspend(() => {
      const fail = (description: string, cause?: unknown) =>
        Effect.fail(
          new HttpClientError.HttpClientError({
            reason: new HttpClientError.InvalidUrlError({
              request: method(""),
              cause,
              description,
            }),
          }),
        )
      if (parameters.some((value) => value === "" || /^(?:\.|%2e){1,2}$/i.test(value))) {
        return fail("Path parameters must be non-empty and cannot be dot segments")
      }
      let path: string
      try {
        path = getPath()
      } catch (cause) {
        return fail("Failed to encode path parameter", cause)
      }
      if (path.split("/").some((segment) => /^(?:\.|%2e){1,2}$/i.test(segment))) {
        return fail("Request paths cannot contain dot segments")
      }
      return Effect.succeed(method(path))
    })
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
      HttpClientRequest.get("/api/board").pipe(
        withResponse(options?.config)(
          HttpClientResponse.matchStatus({
            "2xx": decodeSuccess(GetMergedBoard200),
            orElse: unexpectedStatus,
          }),
        ),
      ),
    drawDiagram: (options) =>
      HttpClientRequest.post("/api/diagram").pipe(
        HttpClientRequest.bodyJsonUnsafe(options.payload),
        withResponse(options.config)(
          HttpClientResponse.matchStatus({
            "2xx": decodeSuccess(DrawDiagram200),
            "422": decodeError("DrawDiagram422", DrawDiagram422),
            orElse: unexpectedStatus,
          }),
        ),
      ),
    getFlow: (options) =>
      HttpClientRequest.get("/api/flow").pipe(
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
            "503": decodeError("GetFlow503", GetFlow503),
            orElse: unexpectedStatus,
          }),
        ),
      ),
    drawFlow: (options) =>
      HttpClientRequest.get("/api/flow/drawing").pipe(
        HttpClientRequest.setUrlParams({
          project: options?.params?.["project"] as any,
          status: options?.params?.["status"] as any,
          task: options?.params?.["task"] as any,
          tag: options?.params?.["tag"] as any,
          width: options?.params?.["width"] as any,
          height: options?.params?.["height"] as any,
        }),
        withResponse(options?.config)(
          HttpClientResponse.matchStatus({
            "2xx": decodeSuccess(DrawFlow200),
            "400": decodeError("DrawFlow400", DrawFlow400),
            "404": decodeError("DrawFlow404", DrawFlow404),
            "422": decodeError("DrawFlow422", DrawFlow422),
            "503": decodeError("DrawFlow503", DrawFlow503),
            orElse: unexpectedStatus,
          }),
        ),
      ),
    listProjects: (options) =>
      HttpClientRequest.get("/api/projects").pipe(
        withResponse(options?.config)(
          HttpClientResponse.matchStatus({
            "2xx": decodeSuccess(ListProjects200),
            orElse: unexpectedStatus,
          }),
        ),
      ),
    registerProject: (options) =>
      HttpClientRequest.post("/api/projects").pipe(
        HttpClientRequest.bodyJsonUnsafe(options.payload),
        withResponse(options.config)(
          HttpClientResponse.matchStatus({
            "200": decodeSuccess(RegisterProject200),
            "201": decodeSuccess(RegisterProject201),
            "400": decodeError("RegisterProject400", RegisterProject400),
            "409": decodeError("RegisterProject409", RegisterProject409),
            "503": decodeError("RegisterProject503", RegisterProject503),
            orElse: unexpectedStatus,
          }),
        ),
      ),
    deleteProject: (project, options) =>
      __makePathRequest(
        HttpClientRequest.delete,
        [project],
        () => "/api/projects/" + __encodePathParam(project) + "",
      ).pipe(
        Effect.flatMap((request) =>
          request.pipe(
            withResponse(options?.config)(
              HttpClientResponse.matchStatus({
                "404": decodeError("DeleteProject404", DeleteProject404),
                "503": decodeError("DeleteProject503", DeleteProject503),
                "204": () => Effect.void,
                orElse: unexpectedStatus,
              }),
            ),
          ),
        ),
      ),
    renameProject: (project, options) =>
      __makePathRequest(
        HttpClientRequest.patch,
        [project],
        () => "/api/projects/" + __encodePathParam(project) + "",
      ).pipe(
        Effect.flatMap((request) =>
          request.pipe(
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
        ),
      ),
    listSessions: (project, options) =>
      __makePathRequest(
        HttpClientRequest.get,
        [project],
        () => "/api/projects/" + __encodePathParam(project) + "/agent/sessions",
      ).pipe(
        Effect.flatMap((request) =>
          request.pipe(
            withResponse(options?.config)(
              HttpClientResponse.matchStatus({
                "2xx": decodeSuccess(ListSessions200),
                "404": decodeError("ListSessions404", ListSessions404),
                "503": decodeError("ListSessions503", ListSessions503),
                orElse: unexpectedStatus,
              }),
            ),
          ),
        ),
      ),
    createSession: (project, options) =>
      __makePathRequest(
        HttpClientRequest.post,
        [project],
        () => "/api/projects/" + __encodePathParam(project) + "/agent/sessions",
      ).pipe(
        Effect.flatMap((request) =>
          request.pipe(
            HttpClientRequest.bodyJsonUnsafe(options.payload),
            withResponse(options.config)(
              HttpClientResponse.matchStatus({
                "2xx": decodeSuccess(CreateSession201),
                "400": decodeError("CreateSession400", CreateSession400),
                "404": decodeError("CreateSession404", CreateSession404),
                "503": decodeError("CreateSession503", CreateSession503),
                orElse: unexpectedStatus,
              }),
            ),
          ),
        ),
      ),
    deleteSession: (project, id, options) =>
      __makePathRequest(
        HttpClientRequest.delete,
        [project, id],
        () => "/api/projects/" + __encodePathParam(project) + "/agent/sessions/" + __encodePathParam(id) + "",
      ).pipe(
        Effect.flatMap((request) =>
          request.pipe(
            withResponse(options?.config)(
              HttpClientResponse.matchStatus({
                "404": decodeError("DeleteSession404", DeleteSession404),
                "503": decodeError("DeleteSession503", DeleteSession503),
                "204": () => Effect.void,
                orElse: unexpectedStatus,
              }),
            ),
          ),
        ),
      ),
    approve: (project, id, approval, options) =>
      __makePathRequest(
        HttpClientRequest.post,
        [project, id, approval],
        () =>
          "/api/projects/" +
          __encodePathParam(project) +
          "/agent/sessions/" +
          __encodePathParam(id) +
          "/approvals/" +
          __encodePathParam(approval) +
          "",
      ).pipe(
        Effect.flatMap((request) =>
          request.pipe(
            HttpClientRequest.bodyJsonUnsafe(options.payload),
            withResponse(options.config)(
              HttpClientResponse.matchStatus({
                "404": decodeError("Approve404", Approve404),
                "503": decodeError("Approve503", Approve503),
                "202": () => Effect.void,
                orElse: unexpectedStatus,
              }),
            ),
          ),
        ),
      ),
    interruptSession: (project, id, options) =>
      __makePathRequest(
        HttpClientRequest.post,
        [project, id],
        () => "/api/projects/" + __encodePathParam(project) + "/agent/sessions/" + __encodePathParam(id) + "/interrupt",
      ).pipe(
        Effect.flatMap((request) =>
          request.pipe(
            withResponse(options?.config)(
              HttpClientResponse.matchStatus({
                "404": decodeError("InterruptSession404", InterruptSession404),
                "503": decodeError("InterruptSession503", InterruptSession503),
                "202": () => Effect.void,
                orElse: unexpectedStatus,
              }),
            ),
          ),
        ),
      ),
    promptSession: (project, id, options) =>
      __makePathRequest(
        HttpClientRequest.post,
        [project, id],
        () => "/api/projects/" + __encodePathParam(project) + "/agent/sessions/" + __encodePathParam(id) + "/prompt",
      ).pipe(
        Effect.flatMap((request) =>
          request.pipe(
            HttpClientRequest.bodyJsonUnsafe(options.payload),
            withResponse(options.config)(
              HttpClientResponse.matchStatus({
                "404": decodeError("PromptSession404", PromptSession404),
                "409": decodeError("PromptSession409", PromptSession409),
                "503": decodeError("PromptSession503", PromptSession503),
                "202": () => Effect.void,
                orElse: unexpectedStatus,
              }),
            ),
          ),
        ),
      ),
    getBoard: (project, options) =>
      __makePathRequest(
        HttpClientRequest.get,
        [project],
        () => "/api/projects/" + __encodePathParam(project) + "/board",
      ).pipe(
        Effect.flatMap((request) =>
          request.pipe(
            withResponse(options?.config)(
              HttpClientResponse.matchStatus({
                "2xx": decodeSuccess(GetBoard200),
                "404": decodeError("GetBoard404", GetBoard404),
                "503": decodeError("GetBoard503", GetBoard503),
                orElse: unexpectedStatus,
              }),
            ),
          ),
        ),
      ),
    projectHistory: (project, options) =>
      __makePathRequest(
        HttpClientRequest.get,
        [project],
        () => "/api/projects/" + __encodePathParam(project) + "/history",
      ).pipe(
        Effect.flatMap((request) =>
          request.pipe(
            HttpClientRequest.setUrlParams({
              before: options?.params?.["before"] as any,
              limit: options?.params?.["limit"] as any,
            }),
            withResponse(options?.config)(
              HttpClientResponse.matchStatus({
                "2xx": decodeSuccess(ProjectHistory200),
                "404": decodeError("ProjectHistory404", ProjectHistory404),
                "503": decodeError("ProjectHistory503", ProjectHistory503),
                orElse: unexpectedStatus,
              }),
            ),
          ),
        ),
      ),
    searchProject: (project, options) =>
      __makePathRequest(
        HttpClientRequest.get,
        [project],
        () => "/api/projects/" + __encodePathParam(project) + "/search",
      ).pipe(
        Effect.flatMap((request) =>
          request.pipe(
            HttpClientRequest.setUrlParams({
              q: options?.params?.["q"] as any,
              fresh: options?.params?.["fresh"] as any,
            }),
            withResponse(options?.config)(
              HttpClientResponse.matchStatus({
                "2xx": decodeSuccess(SearchProject200),
                "404": decodeError("SearchProject404", SearchProject404),
                "503": decodeError("SearchProject503", SearchProject503),
                orElse: unexpectedStatus,
              }),
            ),
          ),
        ),
      ),
    getSync: (project, options) =>
      __makePathRequest(
        HttpClientRequest.get,
        [project],
        () => "/api/projects/" + __encodePathParam(project) + "/sync",
      ).pipe(
        Effect.flatMap((request) =>
          request.pipe(
            withResponse(options?.config)(
              HttpClientResponse.matchStatus({
                "2xx": decodeSuccess(GetSync200),
                "404": decodeError("GetSync404", GetSync404),
                "503": decodeError("GetSync503", GetSync503),
                orElse: unexpectedStatus,
              }),
            ),
          ),
        ),
      ),
    runSync: (project, options) =>
      __makePathRequest(
        HttpClientRequest.post,
        [project],
        () => "/api/projects/" + __encodePathParam(project) + "/sync",
      ).pipe(
        Effect.flatMap((request) =>
          request.pipe(
            withResponse(options?.config)(
              HttpClientResponse.matchStatus({
                "2xx": decodeSuccess(RunSync200),
                "404": decodeError("RunSync404", RunSync404),
                "502": decodeError("RunSync502", RunSync502),
                "503": decodeError("RunSync503", RunSync503),
                orElse: unexpectedStatus,
              }),
            ),
          ),
        ),
      ),
    listTags: (project, options) =>
      __makePathRequest(
        HttpClientRequest.get,
        [project],
        () => "/api/projects/" + __encodePathParam(project) + "/tags",
      ).pipe(
        Effect.flatMap((request) =>
          request.pipe(
            withResponse(options?.config)(
              HttpClientResponse.matchStatus({
                "2xx": decodeSuccess(ListTags200),
                "404": decodeError("ListTags404", ListTags404),
                "422": decodeError("ListTags422", ListTags422),
                "503": decodeError("ListTags503", ListTags503),
                orElse: unexpectedStatus,
              }),
            ),
          ),
        ),
      ),
    createTag: (project, options) =>
      __makePathRequest(
        HttpClientRequest.post,
        [project],
        () => "/api/projects/" + __encodePathParam(project) + "/tags",
      ).pipe(
        Effect.flatMap((request) =>
          request.pipe(
            HttpClientRequest.bodyJsonUnsafe(options.payload),
            withResponse(options.config)(
              HttpClientResponse.matchStatus({
                "2xx": decodeSuccess(CreateTag201),
                "400": decodeError("CreateTag400", CreateTag400),
                "404": decodeError("CreateTag404", CreateTag404),
                "409": decodeError("CreateTag409", CreateTag409),
                "422": decodeError("CreateTag422", CreateTag422),
                "503": decodeError("CreateTag503", CreateTag503),
                orElse: unexpectedStatus,
              }),
            ),
          ),
        ),
      ),
    getTag: (project, name, options) =>
      __makePathRequest(
        HttpClientRequest.get,
        [project, name],
        () => "/api/projects/" + __encodePathParam(project) + "/tags/" + __encodePathParam(name) + "",
      ).pipe(
        Effect.flatMap((request) =>
          request.pipe(
            withResponse(options?.config)(
              HttpClientResponse.matchStatus({
                "2xx": decodeSuccess(GetTag200),
                "400": decodeError("GetTag400", GetTag400),
                "404": decodeError("GetTag404", GetTag404),
                "422": decodeError("GetTag422", GetTag422),
                "503": decodeError("GetTag503", GetTag503),
                orElse: unexpectedStatus,
              }),
            ),
          ),
        ),
      ),
    deleteTag: (project, name, options) =>
      __makePathRequest(
        HttpClientRequest.delete,
        [project, name],
        () => "/api/projects/" + __encodePathParam(project) + "/tags/" + __encodePathParam(name) + "",
      ).pipe(
        Effect.flatMap((request) =>
          request.pipe(
            HttpClientRequest.setUrlParams({ force: options?.params?.["force"] as any }),
            withResponse(options?.config)(
              HttpClientResponse.matchStatus({
                "400": decodeError("DeleteTag400", DeleteTag400),
                "404": decodeError("DeleteTag404", DeleteTag404),
                "409": decodeError("DeleteTag409", DeleteTag409),
                "503": decodeError("DeleteTag503", DeleteTag503),
                "204": () => Effect.void,
                orElse: unexpectedStatus,
              }),
            ),
          ),
        ),
      ),
    patchTag: (project, name, options) =>
      __makePathRequest(
        HttpClientRequest.patch,
        [project, name],
        () => "/api/projects/" + __encodePathParam(project) + "/tags/" + __encodePathParam(name) + "",
      ).pipe(
        Effect.flatMap((request) =>
          request.pipe(
            HttpClientRequest.bodyJsonUnsafe(options.payload),
            withResponse(options.config)(
              HttpClientResponse.matchStatus({
                "2xx": decodeSuccess(PatchTag200),
                "400": decodeError("PatchTag400", PatchTag400),
                "404": decodeError("PatchTag404", PatchTag404),
                "409": decodeError("PatchTag409", PatchTag409),
                "422": decodeError("PatchTag422", PatchTag422),
                "503": decodeError("PatchTag503", PatchTag503),
                orElse: unexpectedStatus,
              }),
            ),
          ),
        ),
      ),
    listTasks: (project, options) =>
      __makePathRequest(
        HttpClientRequest.get,
        [project],
        () => "/api/projects/" + __encodePathParam(project) + "/tasks",
      ).pipe(
        Effect.flatMap((request) =>
          request.pipe(
            HttpClientRequest.setUrlParams({ fresh: options?.params?.["fresh"] as any }),
            withResponse(options?.config)(
              HttpClientResponse.matchStatus({
                "2xx": decodeSuccess(ListTasks200),
                "404": decodeError("ListTasks404", ListTasks404),
                "503": decodeError("ListTasks503", ListTasks503),
                orElse: unexpectedStatus,
              }),
            ),
          ),
        ),
      ),
    createTask: (project, options) =>
      __makePathRequest(
        HttpClientRequest.post,
        [project],
        () => "/api/projects/" + __encodePathParam(project) + "/tasks",
      ).pipe(
        Effect.flatMap((request) =>
          request.pipe(
            HttpClientRequest.bodyJsonUnsafe(options.payload),
            withResponse(options.config)(
              HttpClientResponse.matchStatus({
                "2xx": decodeSuccess(CreateTask201),
                "400": decodeError("CreateTask400", CreateTask400),
                "404": decodeError("CreateTask404", CreateTask404),
                "409": decodeError("CreateTask409", CreateTask409),
                "503": decodeError("CreateTask503", CreateTask503),
                orElse: unexpectedStatus,
              }),
            ),
          ),
        ),
      ),
    getTask: (project, id, options) =>
      __makePathRequest(
        HttpClientRequest.get,
        [project, id],
        () => "/api/projects/" + __encodePathParam(project) + "/tasks/" + __encodePathParam(id) + "",
      ).pipe(
        Effect.flatMap((request) =>
          request.pipe(
            HttpClientRequest.setUrlParams({ fresh: options?.params?.["fresh"] as any }),
            withResponse(options?.config)(
              HttpClientResponse.matchStatus({
                "2xx": decodeSuccess(GetTask200),
                "400": decodeError("GetTask400", GetTask400),
                "404": decodeError("GetTask404", GetTask404),
                "503": decodeError("GetTask503", GetTask503),
                orElse: unexpectedStatus,
              }),
            ),
          ),
        ),
      ),
    deleteTask: (project, id, options) =>
      __makePathRequest(
        HttpClientRequest.delete,
        [project, id],
        () => "/api/projects/" + __encodePathParam(project) + "/tasks/" + __encodePathParam(id) + "",
      ).pipe(
        Effect.flatMap((request) =>
          request.pipe(
            withResponse(options?.config)(
              HttpClientResponse.matchStatus({
                "400": decodeError("DeleteTask400", DeleteTask400),
                "404": decodeError("DeleteTask404", DeleteTask404),
                "503": decodeError("DeleteTask503", DeleteTask503),
                "204": () => Effect.void,
                orElse: unexpectedStatus,
              }),
            ),
          ),
        ),
      ),
    patchTask: (project, id, options) =>
      __makePathRequest(
        HttpClientRequest.patch,
        [project, id],
        () => "/api/projects/" + __encodePathParam(project) + "/tasks/" + __encodePathParam(id) + "",
      ).pipe(
        Effect.flatMap((request) =>
          request.pipe(
            HttpClientRequest.bodyJsonUnsafe(options.payload),
            withResponse(options.config)(
              HttpClientResponse.matchStatus({
                "2xx": decodeSuccess(PatchTask200),
                "400": decodeError("PatchTask400", PatchTask400),
                "404": decodeError("PatchTask404", PatchTask404),
                "409": decodeError("PatchTask409", PatchTask409),
                "503": decodeError("PatchTask503", PatchTask503),
                orElse: unexpectedStatus,
              }),
            ),
          ),
        ),
      ),
    listComments: (project, id, options) =>
      __makePathRequest(
        HttpClientRequest.get,
        [project, id],
        () => "/api/projects/" + __encodePathParam(project) + "/tasks/" + __encodePathParam(id) + "/comments",
      ).pipe(
        Effect.flatMap((request) =>
          request.pipe(
            HttpClientRequest.setUrlParams({ fresh: options?.params?.["fresh"] as any }),
            withResponse(options?.config)(
              HttpClientResponse.matchStatus({
                "2xx": decodeSuccess(ListComments200),
                "400": decodeError("ListComments400", ListComments400),
                "404": decodeError("ListComments404", ListComments404),
                "503": decodeError("ListComments503", ListComments503),
                orElse: unexpectedStatus,
              }),
            ),
          ),
        ),
      ),
    addComment: (project, id, options) =>
      __makePathRequest(
        HttpClientRequest.post,
        [project, id],
        () => "/api/projects/" + __encodePathParam(project) + "/tasks/" + __encodePathParam(id) + "/comments",
      ).pipe(
        Effect.flatMap((request) =>
          request.pipe(
            HttpClientRequest.bodyJsonUnsafe(options.payload),
            withResponse(options.config)(
              HttpClientResponse.matchStatus({
                "2xx": decodeSuccess(AddComment201),
                "400": decodeError("AddComment400", AddComment400),
                "404": decodeError("AddComment404", AddComment404),
                "503": decodeError("AddComment503", AddComment503),
                orElse: unexpectedStatus,
              }),
            ),
          ),
        ),
      ),
    writeTaskFile: (project, id, options) =>
      __makePathRequest(
        HttpClientRequest.put,
        [project, id],
        () => "/api/projects/" + __encodePathParam(project) + "/tasks/" + __encodePathParam(id) + "/file",
      ).pipe(
        Effect.flatMap((request) =>
          request.pipe(
            HttpClientRequest.bodyJsonUnsafe(options.payload),
            withResponse(options.config)(
              HttpClientResponse.matchStatus({
                "2xx": decodeSuccess(WriteTaskFile200),
                "400": decodeError("WriteTaskFile400", WriteTaskFile400),
                "404": decodeError("WriteTaskFile404", WriteTaskFile404),
                "409": decodeError("WriteTaskFile409", WriteTaskFile409),
                "503": decodeError("WriteTaskFile503", WriteTaskFile503),
                orElse: unexpectedStatus,
              }),
            ),
          ),
        ),
      ),
    taskHistory: (project, id, options) =>
      __makePathRequest(
        HttpClientRequest.get,
        [project, id],
        () => "/api/projects/" + __encodePathParam(project) + "/tasks/" + __encodePathParam(id) + "/history",
      ).pipe(
        Effect.flatMap((request) =>
          request.pipe(
            HttpClientRequest.setUrlParams({
              before: options?.params?.["before"] as any,
              limit: options?.params?.["limit"] as any,
            }),
            withResponse(options?.config)(
              HttpClientResponse.matchStatus({
                "2xx": decodeSuccess(TaskHistory200),
                "400": decodeError("TaskHistory400", TaskHistory400),
                "404": decodeError("TaskHistory404", TaskHistory404),
                "503": decodeError("TaskHistory503", TaskHistory503),
                orElse: unexpectedStatus,
              }),
            ),
          ),
        ),
      ),
    resolveConflict: (project, id, options) =>
      __makePathRequest(
        HttpClientRequest.post,
        [project, id],
        () => "/api/projects/" + __encodePathParam(project) + "/tasks/" + __encodePathParam(id) + "/resolve",
      ).pipe(
        Effect.flatMap((request) =>
          request.pipe(
            HttpClientRequest.bodyJsonUnsafe(options.payload),
            withResponse(options.config)(
              HttpClientResponse.matchStatus({
                "2xx": decodeSuccess(ResolveConflict200),
                "400": decodeError("ResolveConflict400", ResolveConflict400),
                "404": decodeError("ResolveConflict404", ResolveConflict404),
                "409": decodeError("ResolveConflict409", ResolveConflict409),
                "503": decodeError("ResolveConflict503", ResolveConflict503),
                orElse: unexpectedStatus,
              }),
            ),
          ),
        ),
      ),
    taskRevision: (project, id, revision, options) =>
      __makePathRequest(
        HttpClientRequest.get,
        [project, id, revision],
        () =>
          "/api/projects/" +
          __encodePathParam(project) +
          "/tasks/" +
          __encodePathParam(id) +
          "/revisions/" +
          __encodePathParam(revision) +
          "",
      ).pipe(
        Effect.flatMap((request) =>
          request.pipe(
            withResponse(options?.config)(
              HttpClientResponse.matchStatus({
                "2xx": decodeSuccess(TaskRevision200),
                "400": decodeError("TaskRevision400", TaskRevision400),
                "404": decodeError("TaskRevision404", TaskRevision404),
                "503": decodeError("TaskRevision503", TaskRevision503),
                orElse: unexpectedStatus,
              }),
            ),
          ),
        ),
      ),
    getTaskTree: (project, id, options) =>
      __makePathRequest(
        HttpClientRequest.get,
        [project, id],
        () => "/api/projects/" + __encodePathParam(project) + "/tasks/" + __encodePathParam(id) + "/tree",
      ).pipe(
        Effect.flatMap((request) =>
          request.pipe(
            HttpClientRequest.setUrlParams({
              fresh: options?.params?.["fresh"] as any,
              depth: options?.params?.["depth"] as any,
            }),
            withResponse(options?.config)(
              HttpClientResponse.matchStatus({
                "2xx": decodeSuccess(GetTaskTree200),
                "400": decodeError("GetTaskTree400", GetTaskTree400),
                "404": decodeError("GetTaskTree404", GetTaskTree404),
                "503": decodeError("GetTaskTree503", GetTaskTree503),
                orElse: unexpectedStatus,
              }),
            ),
          ),
        ),
      ),
    searchAll: (options) =>
      HttpClientRequest.get("/api/search").pipe(
        HttpClientRequest.setUrlParams({ q: options?.params?.["q"] as any, fresh: options?.params?.["fresh"] as any }),
        withResponse(options?.config)(
          HttpClientResponse.matchStatus({
            "2xx": decodeSuccess(SearchAll200),
            orElse: unexpectedStatus,
          }),
        ),
      ),
    health: (options) =>
      HttpClientRequest.get("/health").pipe(
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
    HttpClientError.HttpClientError | SchemaError
  >
  readonly drawDiagram: <Config extends OperationConfig>(options: {
    readonly payload: typeof DrawDiagramRequestJson.Encoded
    readonly config?: Config | undefined
  }) => Effect.Effect<
    WithOptionalResponse<typeof DrawDiagram200.Type, Config>,
    HttpClientError.HttpClientError | SchemaError | TasksClientError<"DrawDiagram422", typeof DrawDiagram422.Type>
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
    | TasksClientError<"GetFlow503", typeof GetFlow503.Type>
  >
  readonly drawFlow: <Config extends OperationConfig>(
    options:
      | { readonly params?: typeof DrawFlowParams.Encoded | undefined; readonly config?: Config | undefined }
      | undefined,
  ) => Effect.Effect<
    WithOptionalResponse<typeof DrawFlow200.Type, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"DrawFlow400", typeof DrawFlow400.Type>
    | TasksClientError<"DrawFlow404", typeof DrawFlow404.Type>
    | TasksClientError<"DrawFlow422", typeof DrawFlow422.Type>
    | TasksClientError<"DrawFlow503", typeof DrawFlow503.Type>
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
    | TasksClientError<"RegisterProject409", typeof RegisterProject409.Type>
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
  readonly listSessions: <Config extends OperationConfig>(
    project: string,
    options: { readonly config?: Config | undefined } | undefined,
  ) => Effect.Effect<
    WithOptionalResponse<typeof ListSessions200.Type, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"ListSessions404", typeof ListSessions404.Type>
    | TasksClientError<"ListSessions503", typeof ListSessions503.Type>
  >
  readonly createSession: <Config extends OperationConfig>(
    project: string,
    options: { readonly payload: typeof CreateSessionRequestJson.Encoded; readonly config?: Config | undefined },
  ) => Effect.Effect<
    WithOptionalResponse<typeof CreateSession201.Type, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"CreateSession400", typeof CreateSession400.Type>
    | TasksClientError<"CreateSession404", typeof CreateSession404.Type>
    | TasksClientError<"CreateSession503", typeof CreateSession503.Type>
  >
  readonly deleteSession: <Config extends OperationConfig>(
    project: string,
    id: string,
    options: { readonly config?: Config | undefined } | undefined,
  ) => Effect.Effect<
    WithOptionalResponse<void, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"DeleteSession404", typeof DeleteSession404.Type>
    | TasksClientError<"DeleteSession503", typeof DeleteSession503.Type>
  >
  readonly approve: <Config extends OperationConfig>(
    project: string,
    id: string,
    approval: string,
    options: { readonly payload: typeof ApproveRequestJson.Encoded; readonly config?: Config | undefined },
  ) => Effect.Effect<
    WithOptionalResponse<void, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"Approve404", typeof Approve404.Type>
    | TasksClientError<"Approve503", typeof Approve503.Type>
  >
  readonly interruptSession: <Config extends OperationConfig>(
    project: string,
    id: string,
    options: { readonly config?: Config | undefined } | undefined,
  ) => Effect.Effect<
    WithOptionalResponse<void, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"InterruptSession404", typeof InterruptSession404.Type>
    | TasksClientError<"InterruptSession503", typeof InterruptSession503.Type>
  >
  readonly promptSession: <Config extends OperationConfig>(
    project: string,
    id: string,
    options: { readonly payload: typeof PromptSessionRequestJson.Encoded; readonly config?: Config | undefined },
  ) => Effect.Effect<
    WithOptionalResponse<void, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"PromptSession404", typeof PromptSession404.Type>
    | TasksClientError<"PromptSession409", typeof PromptSession409.Type>
    | TasksClientError<"PromptSession503", typeof PromptSession503.Type>
  >
  readonly getBoard: <Config extends OperationConfig>(
    project: string,
    options: { readonly config?: Config | undefined } | undefined,
  ) => Effect.Effect<
    WithOptionalResponse<typeof GetBoard200.Type, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"GetBoard404", typeof GetBoard404.Type>
    | TasksClientError<"GetBoard503", typeof GetBoard503.Type>
  >
  readonly projectHistory: <Config extends OperationConfig>(
    project: string,
    options:
      | { readonly params?: typeof ProjectHistoryParams.Encoded | undefined; readonly config?: Config | undefined }
      | undefined,
  ) => Effect.Effect<
    WithOptionalResponse<typeof ProjectHistory200.Type, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"ProjectHistory404", typeof ProjectHistory404.Type>
    | TasksClientError<"ProjectHistory503", typeof ProjectHistory503.Type>
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
    | TasksClientError<"SearchProject503", typeof SearchProject503.Type>
  >
  readonly getSync: <Config extends OperationConfig>(
    project: string,
    options: { readonly config?: Config | undefined } | undefined,
  ) => Effect.Effect<
    WithOptionalResponse<typeof GetSync200.Type, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"GetSync404", typeof GetSync404.Type>
    | TasksClientError<"GetSync503", typeof GetSync503.Type>
  >
  readonly runSync: <Config extends OperationConfig>(
    project: string,
    options: { readonly config?: Config | undefined } | undefined,
  ) => Effect.Effect<
    WithOptionalResponse<typeof RunSync200.Type, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"RunSync404", typeof RunSync404.Type>
    | TasksClientError<"RunSync502", typeof RunSync502.Type>
    | TasksClientError<"RunSync503", typeof RunSync503.Type>
  >
  readonly listTags: <Config extends OperationConfig>(
    project: string,
    options: { readonly config?: Config | undefined } | undefined,
  ) => Effect.Effect<
    WithOptionalResponse<typeof ListTags200.Type, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"ListTags404", typeof ListTags404.Type>
    | TasksClientError<"ListTags422", typeof ListTags422.Type>
    | TasksClientError<"ListTags503", typeof ListTags503.Type>
  >
  readonly createTag: <Config extends OperationConfig>(
    project: string,
    options: { readonly payload: typeof CreateTagRequestJson.Encoded; readonly config?: Config | undefined },
  ) => Effect.Effect<
    WithOptionalResponse<typeof CreateTag201.Type, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"CreateTag400", typeof CreateTag400.Type>
    | TasksClientError<"CreateTag404", typeof CreateTag404.Type>
    | TasksClientError<"CreateTag409", typeof CreateTag409.Type>
    | TasksClientError<"CreateTag422", typeof CreateTag422.Type>
    | TasksClientError<"CreateTag503", typeof CreateTag503.Type>
  >
  readonly getTag: <Config extends OperationConfig>(
    project: string,
    name: string,
    options: { readonly config?: Config | undefined } | undefined,
  ) => Effect.Effect<
    WithOptionalResponse<typeof GetTag200.Type, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"GetTag400", typeof GetTag400.Type>
    | TasksClientError<"GetTag404", typeof GetTag404.Type>
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
    | TasksClientError<"DeleteTag503", typeof DeleteTag503.Type>
  >
  readonly patchTag: <Config extends OperationConfig>(
    project: string,
    name: string,
    options: { readonly payload: typeof PatchTagRequestJson.Encoded; readonly config?: Config | undefined },
  ) => Effect.Effect<
    WithOptionalResponse<typeof PatchTag200.Type, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"PatchTag400", typeof PatchTag400.Type>
    | TasksClientError<"PatchTag404", typeof PatchTag404.Type>
    | TasksClientError<"PatchTag409", typeof PatchTag409.Type>
    | TasksClientError<"PatchTag422", typeof PatchTag422.Type>
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
    | TasksClientError<"ListTasks404", typeof ListTasks404.Type>
    | TasksClientError<"ListTasks503", typeof ListTasks503.Type>
  >
  readonly createTask: <Config extends OperationConfig>(
    project: string,
    options: { readonly payload: typeof CreateTaskRequestJson.Encoded; readonly config?: Config | undefined },
  ) => Effect.Effect<
    WithOptionalResponse<typeof CreateTask201.Type, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"CreateTask400", typeof CreateTask400.Type>
    | TasksClientError<"CreateTask404", typeof CreateTask404.Type>
    | TasksClientError<"CreateTask409", typeof CreateTask409.Type>
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
    | TasksClientError<"GetTask503", typeof GetTask503.Type>
  >
  readonly deleteTask: <Config extends OperationConfig>(
    project: string,
    id: string,
    options: { readonly config?: Config | undefined } | undefined,
  ) => Effect.Effect<
    WithOptionalResponse<void, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"DeleteTask400", typeof DeleteTask400.Type>
    | TasksClientError<"DeleteTask404", typeof DeleteTask404.Type>
    | TasksClientError<"DeleteTask503", typeof DeleteTask503.Type>
  >
  readonly patchTask: <Config extends OperationConfig>(
    project: string,
    id: string,
    options: { readonly payload: typeof PatchTaskRequestJson.Encoded; readonly config?: Config | undefined },
  ) => Effect.Effect<
    WithOptionalResponse<typeof PatchTask200.Type, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"PatchTask400", typeof PatchTask400.Type>
    | TasksClientError<"PatchTask404", typeof PatchTask404.Type>
    | TasksClientError<"PatchTask409", typeof PatchTask409.Type>
    | TasksClientError<"PatchTask503", typeof PatchTask503.Type>
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
    | TasksClientError<"ListComments503", typeof ListComments503.Type>
  >
  readonly addComment: <Config extends OperationConfig>(
    project: string,
    id: string,
    options: { readonly payload: typeof AddCommentRequestJson.Encoded; readonly config?: Config | undefined },
  ) => Effect.Effect<
    WithOptionalResponse<typeof AddComment201.Type, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"AddComment400", typeof AddComment400.Type>
    | TasksClientError<"AddComment404", typeof AddComment404.Type>
    | TasksClientError<"AddComment503", typeof AddComment503.Type>
  >
  readonly writeTaskFile: <Config extends OperationConfig>(
    project: string,
    id: string,
    options: { readonly payload: typeof WriteTaskFileRequestJson.Encoded; readonly config?: Config | undefined },
  ) => Effect.Effect<
    WithOptionalResponse<typeof WriteTaskFile200.Type, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"WriteTaskFile400", typeof WriteTaskFile400.Type>
    | TasksClientError<"WriteTaskFile404", typeof WriteTaskFile404.Type>
    | TasksClientError<"WriteTaskFile409", typeof WriteTaskFile409.Type>
    | TasksClientError<"WriteTaskFile503", typeof WriteTaskFile503.Type>
  >
  readonly taskHistory: <Config extends OperationConfig>(
    project: string,
    id: string,
    options:
      | { readonly params?: typeof TaskHistoryParams.Encoded | undefined; readonly config?: Config | undefined }
      | undefined,
  ) => Effect.Effect<
    WithOptionalResponse<typeof TaskHistory200.Type, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"TaskHistory400", typeof TaskHistory400.Type>
    | TasksClientError<"TaskHistory404", typeof TaskHistory404.Type>
    | TasksClientError<"TaskHistory503", typeof TaskHistory503.Type>
  >
  readonly resolveConflict: <Config extends OperationConfig>(
    project: string,
    id: string,
    options: { readonly payload: typeof ResolveConflictRequestJson.Encoded; readonly config?: Config | undefined },
  ) => Effect.Effect<
    WithOptionalResponse<typeof ResolveConflict200.Type, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"ResolveConflict400", typeof ResolveConflict400.Type>
    | TasksClientError<"ResolveConflict404", typeof ResolveConflict404.Type>
    | TasksClientError<"ResolveConflict409", typeof ResolveConflict409.Type>
    | TasksClientError<"ResolveConflict503", typeof ResolveConflict503.Type>
  >
  readonly taskRevision: <Config extends OperationConfig>(
    project: string,
    id: string,
    revision: string,
    options: { readonly config?: Config | undefined } | undefined,
  ) => Effect.Effect<
    WithOptionalResponse<typeof TaskRevision200.Type, Config>,
    | HttpClientError.HttpClientError
    | SchemaError
    | TasksClientError<"TaskRevision400", typeof TaskRevision400.Type>
    | TasksClientError<"TaskRevision404", typeof TaskRevision404.Type>
    | TasksClientError<"TaskRevision503", typeof TaskRevision503.Type>
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
    | TasksClientError<"GetTaskTree503", typeof GetTaskTree503.Type>
  >
  readonly searchAll: <Config extends OperationConfig>(
    options:
      | { readonly params?: typeof SearchAllParams.Encoded | undefined; readonly config?: Config | undefined }
      | undefined,
  ) => Effect.Effect<
    WithOptionalResponse<typeof SearchAll200.Type, Config>,
    HttpClientError.HttpClientError | SchemaError
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
