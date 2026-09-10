import { Schema } from "effect"
import { describe, expect, it } from "vitest"

import { AgentEvent, SessionEvent } from "../src/lib/agent-events"

const decodeSession = Schema.decodeUnknownSync(SessionEvent)
const decodeAgent = Schema.decodeUnknownSync(AgentEvent)

const usage = {
  input_tokens: 10,
  cached_input_tokens: 4,
  cache_write_tokens: 2,
  output_tokens: 5,
  reasoning_tokens: 1,
  cost_usd: 0.0123,
}

// What `serde` writes for each variant of `op_agent::AgentEvent`: an internally tagged enum, with a
// newtype variant's struct spread beside the tag.
const AGENT_EVENTS: ReadonlyArray<Record<string, unknown>> = [
  { event: "ready", session: "abc", cwd: "/tmp/w", model: "claude-opus-5", tools: ["Read"] },
  { event: "turn_started", turn: "turn-1", prompt: "hi" },
  { event: "thinking", item: "item-1", delta: "th" },
  { event: "thinking_ended", item: "item-1", text: "thought" },
  { event: "message", item: "item-1", delta: "me" },
  { event: "message_ended", item: "item-1", text: "message" },
  { event: "result_delta", delta: "r" },
  { event: "result_ready", value: { ok: true } },
  { event: "tool_started", item: "item-1", name: "Read", input: { file_path: "/a/b.rs" } },
  { event: "tool_output", item: "item-1", delta: "out" },
  { event: "tool_ended", item: "item-1", output: "done", failed: false },
  { event: "approval_requested", id: "ap-1", item: "item-1", tool: "Bash", input: { command: "ls -la" }, reason: null },
  { event: "approval_resolved", id: "ap-1" },
  { event: "usage_updated", turn: usage, session: usage, context_window: 200000 },
  { event: "rate_limit", windows: [{ name: "5h", used_percent: 12.5, resets_at: 1700000000 }] },
  { event: "budget_exhausted", session: usage },
  { event: "turn_ended", turn: "turn-1", stop: "completed" },
  { event: "failed", message: "boom", retrying: true },
  { event: "exited", code: 0 },
]

const SNAPSHOT = {
  kind: "snapshot",
  id: "s1",
  project: "openplan",
  agent: "claude_code",
  task: "OPP-1",
  branch: "openplan/rolling-updates",
  cwd: "/tmp/w",
  status: { kind: "running" },
  started_at: "2026-09-10T01:15:49.880024Z",
  transcript: {
    info: { session: "abc", cwd: "/tmp/w", model: "claude-opus-5", tools: ["Read"] },
    status: { kind: "running" },
    turn: "turn-1",
    entries: [
      { kind: "prompt", text: "hi" },
      { kind: "thinking", item: "item-1", text: "message", done: true },
      {
        kind: "tool",
        item: "item-1",
        name: "Read",
        input: { file_path: "/a/b.rs" },
        output: "done",
        done: true,
        failed: false,
      },
      { kind: "message", item: "item-2", text: "", done: false },
      { kind: "failure", message: "boom", retrying: false },
    ],
    approvals: [{ id: "ap-1", item: "item-1", tool: "Bash", input: { command: "ls -la" }, reason: null }],
    result_text: "r",
    result: { ok: true },
    usage: { ...usage, cost_usd: null },
    context_window: null,
    rate_limit: null,
    over_budget: false,
  },
}

describe("AgentEvent", () => {
  it("decodes every kind serde writes", () => {
    for (const json of AGENT_EVENTS) {
      expect(decodeAgent(json)).toEqual(json)
    }
  })

  it("refuses a kind it does not know", () => {
    expect(() => decodeAgent({ event: "danced", item: "x" })).toThrow()
  })
})

describe("SessionEvent", () => {
  it("decodes the snapshot with the transcript inside it", () => {
    const decoded = decodeSession(SNAPSHOT)
    expect(decoded.kind).toBe("snapshot")
    expect(decoded).toEqual(SNAPSHOT)
  })

  it("decodes every agent event beside its own tag", () => {
    for (const json of AGENT_EVENTS) {
      const decoded = decodeSession({ kind: "agent", ...json })
      expect(decoded).toEqual({ kind: "agent", ...json })
    }
  })

  it("decodes the task binding", () => {
    expect(decodeSession({ kind: "task", id: "OPP-1" })).toEqual({ kind: "task", id: "OPP-1" })
  })

  it("decodes every status a session reports", () => {
    for (const status of [
      { kind: "starting" },
      { kind: "idle" },
      { kind: "running" },
      { kind: "exited", code: null },
    ]) {
      expect(decodeSession({ ...SNAPSHOT, status }).kind).toBe("snapshot")
    }
  })
})
