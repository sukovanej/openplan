import { describe, expect, it } from "vitest"

import type { AgentEvent } from "../src/lib/agent-events"
import { applyAgentEvent, emptyTranscript, type Transcript } from "../src/lib/agent-transcript"

const ready: AgentEvent = { event: "ready", session: "abc", cwd: "/tmp/w", model: "claude-opus-5", tools: [] }
const started: AgentEvent = { event: "turn_started", turn: "t1", prompt: "Write a task" }
const ended: AgentEvent = { event: "turn_ended", turn: "t1", stop: "completed" }

function fold(events: ReadonlyArray<AgentEvent>, from: Transcript = emptyTranscript): Transcript {
  return events.reduce(applyAgentEvent, from)
}

describe("applyAgentEvent", () => {
  it("joins the deltas of one item into one entry", () => {
    const transcript = fold([
      started,
      { event: "message", item: "m1", delta: "Hel" },
      { event: "message", item: "m1", delta: "lo" },
      { event: "thinking", item: "th1", delta: "hm" },
    ])
    expect(transcript.entries).toEqual([
      { kind: "prompt", text: "Write a task" },
      { kind: "message", item: "m1", text: "Hello", done: false },
      { kind: "thinking", item: "th1", text: "hm", done: false },
    ])
  })

  it("replaces the deltas with the ended text and marks the entry done", () => {
    const transcript = fold([
      { event: "message", item: "m1", delta: "Hel" },
      { event: "message_ended", item: "m1", text: "Hello, world" },
      { event: "thinking", item: "th1", delta: "h" },
      { event: "thinking_ended", item: "th1", text: "hmm" },
    ])
    expect(transcript.entries).toEqual([
      { kind: "message", item: "m1", text: "Hello, world", done: true },
      { kind: "thinking", item: "th1", text: "hmm", done: true },
    ])
  })

  it("keeps the streamed tool output when the end reports none, and takes it when it does", () => {
    const start: AgentEvent = { event: "tool_started", item: "t1", name: "Bash", input: { command: "ls" } }
    const streamed = fold([
      start,
      { event: "tool_output", item: "t1", delta: "a\n" },
      { event: "tool_output", item: "t1", delta: "b\n" },
    ])
    const kept = applyAgentEvent(streamed, { event: "tool_ended", item: "t1", output: "", failed: false })
    expect(kept.entries[0]).toMatchObject({ kind: "tool", output: "a\nb\n", done: true, failed: false })
    const replaced = applyAgentEvent(streamed, { event: "tool_ended", item: "t1", output: "whole", failed: true })
    expect(replaced.entries[0]).toMatchObject({ kind: "tool", output: "whole", done: true, failed: true })
  })

  it("holds an approval until it is resolved", () => {
    const asked = fold([
      started,
      { event: "approval_requested", id: "ap1", item: "t1", tool: "Bash", input: { command: "rm x" }, reason: null },
    ])
    expect(asked.approvals.map((request) => request.id)).toEqual(["ap1"])
    expect(applyAgentEvent(asked, { event: "approval_resolved", id: "ap1" }).approvals).toEqual([])
  })

  it("drops an open approval when the turn ends", () => {
    const asked = fold([
      started,
      { event: "approval_requested", id: "ap1", item: null, tool: "Bash", input: {}, reason: null },
      { event: "message", item: "m1", delta: "x" },
    ])
    const settled = applyAgentEvent(asked, ended)
    expect(settled.approvals).toEqual([])
    expect(settled.entries[1]).toMatchObject({ kind: "message", done: true })
    expect(settled.turn).toBeNull()
  })

  it("moves the status from starting to idle to running to idle to exited", () => {
    const statuses: Array<string> = [emptyTranscript.status.kind]
    let transcript = emptyTranscript
    for (const event of [ready, started, ended, { event: "exited", code: 1 } as const]) {
      transcript = applyAgentEvent(transcript, event)
      statuses.push(transcript.status.kind)
    }
    expect(statuses).toEqual(["starting", "idle", "running", "idle", "exited"])
    expect(transcript.status).toEqual({ kind: "exited", code: 1 })
    expect(transcript.info?.model).toBe("claude-opus-5")
  })

  it("carries the session usage, the budget, and a failure", () => {
    const usage = {
      input_tokens: 5,
      cached_input_tokens: 0,
      cache_write_tokens: 0,
      output_tokens: 2,
      reasoning_tokens: 0,
      cost_usd: 0.01,
    }
    const transcript = fold([
      { event: "usage_updated", turn: usage, session: usage, context_window: 100 },
      { event: "budget_exhausted", session: { ...usage, output_tokens: 9 } },
      { event: "failed", message: "rate limited", retrying: true },
    ])
    expect(transcript.usage.output_tokens).toBe(9)
    expect(transcript.context_window).toBe(100)
    expect(transcript.over_budget).toBe(true)
    expect(transcript.entries).toEqual([{ kind: "failure", message: "rate limited", retrying: true }])
  })

  it("leaves the transcript it was given alone", () => {
    const before = fold([started])
    const entries = before.entries
    applyAgentEvent(before, { event: "message", item: "m1", delta: "x" })
    expect(before.entries).toBe(entries)
    expect(before.entries).toHaveLength(1)
  })
})
