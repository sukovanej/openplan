import { Schema, Tuple } from "effect"

// The wire shapes of `op-agent` and `op-server::agent`, written by hand the way `ChangeEvent` is.
// A newtype id is its string, an `Option` is `null`, and an internally tagged variant that wraps a
// struct spreads that struct's fields beside its tag.

export const Usage = Schema.Struct({
  input_tokens: Schema.Number,
  cached_input_tokens: Schema.Number,
  cache_write_tokens: Schema.Number,
  output_tokens: Schema.Number,
  reasoning_tokens: Schema.Number,
  cost_usd: Schema.NullOr(Schema.Number),
})
export type Usage = typeof Usage.Type

export const SessionInfo = Schema.Struct({
  session: Schema.String,
  cwd: Schema.String,
  model: Schema.NullOr(Schema.String),
  tools: Schema.Array(Schema.String),
})
export type SessionInfo = typeof SessionInfo.Type

export const Status = Schema.Union([
  Schema.Struct({ kind: Schema.Literal("starting") }),
  Schema.Struct({ kind: Schema.Literal("idle") }),
  Schema.Struct({ kind: Schema.Literal("running") }),
  Schema.Struct({ kind: Schema.Literal("exited"), code: Schema.NullOr(Schema.Number) }),
])
export type Status = typeof Status.Type

export const Entry = Schema.Union([
  Schema.Struct({ kind: Schema.Literal("prompt"), text: Schema.String }),
  Schema.Struct({ kind: Schema.Literal("thinking"), item: Schema.String, text: Schema.String, done: Schema.Boolean }),
  Schema.Struct({ kind: Schema.Literal("message"), item: Schema.String, text: Schema.String, done: Schema.Boolean }),
  Schema.Struct({
    kind: Schema.Literal("tool"),
    item: Schema.String,
    name: Schema.String,
    input: Schema.Unknown,
    output: Schema.String,
    done: Schema.Boolean,
    failed: Schema.Boolean,
  }),
  Schema.Struct({ kind: Schema.Literal("failure"), message: Schema.String, retrying: Schema.Boolean }),
])
export type Entry = typeof Entry.Type

export const ApprovalRequest = Schema.Struct({
  id: Schema.String,
  item: Schema.NullOr(Schema.String),
  tool: Schema.String,
  input: Schema.Unknown,
  reason: Schema.NullOr(Schema.String),
})
export type ApprovalRequest = typeof ApprovalRequest.Type

export const RateLimit = Schema.Struct({
  windows: Schema.Array(
    Schema.Struct({
      name: Schema.String,
      used_percent: Schema.Number,
      resets_at: Schema.NullOr(Schema.Number),
    }),
  ),
})
export type RateLimit = typeof RateLimit.Type

export const Transcript = Schema.Struct({
  info: Schema.NullOr(SessionInfo),
  status: Status,
  turn: Schema.NullOr(Schema.String),
  entries: Schema.Array(Entry),
  approvals: Schema.Array(ApprovalRequest),
  result_text: Schema.String,
  result: Schema.Unknown,
  usage: Usage,
  context_window: Schema.NullOr(Schema.Number),
  rate_limit: Schema.NullOr(RateLimit),
  over_budget: Schema.Boolean,
})
export type Transcript = typeof Transcript.Type

export const AgentKind = Schema.Literals(["claude_code", "codex"])
export type AgentKind = typeof AgentKind.Type

export const SessionView = Schema.Struct({
  id: Schema.String,
  project: Schema.String,
  agent: AgentKind,
  task: Schema.NullOr(Schema.String),
  branch: Schema.String,
  cwd: Schema.String,
  status: Status,
  started_at: Schema.String,
  transcript: Transcript,
})
export type SessionView = typeof SessionView.Type

export const AgentEvent = Schema.Union([
  Schema.Struct({ event: Schema.Literal("ready"), ...SessionInfo.fields }),
  Schema.Struct({ event: Schema.Literal("turn_started"), turn: Schema.String, prompt: Schema.String }),
  Schema.Struct({ event: Schema.Literal("thinking"), item: Schema.String, delta: Schema.String }),
  Schema.Struct({ event: Schema.Literal("thinking_ended"), item: Schema.String, text: Schema.String }),
  Schema.Struct({ event: Schema.Literal("message"), item: Schema.String, delta: Schema.String }),
  Schema.Struct({ event: Schema.Literal("message_ended"), item: Schema.String, text: Schema.String }),
  Schema.Struct({ event: Schema.Literal("result_delta"), delta: Schema.String }),
  Schema.Struct({ event: Schema.Literal("result_ready"), value: Schema.Unknown }),
  Schema.Struct({
    event: Schema.Literal("tool_started"),
    item: Schema.String,
    name: Schema.String,
    input: Schema.Unknown,
  }),
  Schema.Struct({ event: Schema.Literal("tool_output"), item: Schema.String, delta: Schema.String }),
  Schema.Struct({
    event: Schema.Literal("tool_ended"),
    item: Schema.String,
    output: Schema.String,
    failed: Schema.Boolean,
  }),
  Schema.Struct({ event: Schema.Literal("approval_requested"), ...ApprovalRequest.fields }),
  Schema.Struct({ event: Schema.Literal("approval_resolved"), id: Schema.String }),
  Schema.Struct({
    event: Schema.Literal("usage_updated"),
    turn: Usage,
    session: Usage,
    context_window: Schema.NullOr(Schema.Number),
  }),
  Schema.Struct({ event: Schema.Literal("rate_limit"), ...RateLimit.fields }),
  Schema.Struct({ event: Schema.Literal("budget_exhausted"), session: Usage }),
  Schema.Struct({
    event: Schema.Literal("turn_ended"),
    turn: Schema.String,
    stop: Schema.Literals(["completed", "interrupted", "failed"]),
  }),
  Schema.Struct({ event: Schema.Literal("failed"), message: Schema.String, retrying: Schema.Boolean }),
  Schema.Struct({ event: Schema.Literal("exited"), code: Schema.NullOr(Schema.Number) }),
])
export type AgentEvent = typeof AgentEvent.Type

export const SessionEvent = Schema.Union([
  Schema.Struct({ kind: Schema.Literal("snapshot"), ...SessionView.fields }),
  ...AgentEvent.mapMembers(Tuple.map(Schema.fieldsAssign({ kind: Schema.Literal("agent") }))).members,
  Schema.Struct({ kind: Schema.Literal("task"), id: Schema.String }),
])
export type SessionEvent = typeof SessionEvent.Type
