import { describe, expect, it } from "vitest"

import { chatItems, latest, newest, phrase } from "../src/lib/agent-activity"
import type { Entry, Transcript } from "../src/lib/agent-events"
import { emptyTranscript } from "../src/lib/agent-transcript"

const tool = (name: string, input: unknown, failed = false, done = true): Entry => ({
  kind: "tool",
  item: name,
  name,
  input,
  output: "",
  done,
  failed,
})

const thinking: Entry = { kind: "thinking", item: "th", text: "hm", done: true }
const message = (text: string, done = true): Entry => ({ kind: "message", item: text, text, done })

describe("phrase", () => {
  it("says what each tool does, without its name or its input", () => {
    expect(phrase(thinking)).toBe("Thinking")
    expect(phrase(tool("Read", { file_path: "/repo/crates/op-server/src/agent.rs" }))).toBe("Reading agent.rs")
    expect(phrase(tool("Read", { path: "src/lib.rs" }))).toBe("Reading lib.rs")
    expect(phrase(tool("Grep", { pattern: "x" }))).toBe("Searching")
    expect(phrase(tool("Glob", { pattern: "**/*.rs" }))).toBe("Searching")
    expect(phrase(tool("web_search", { query: "x" }))).toBe("Searching")
    expect(phrase(tool("Edit", { file_path: "/w/.plan/tasks/00115-x.md" }))).toBe("Writing 00115-x.md")
    expect(phrase(tool("Write", { file_path: "/w/.plan/tasks/00115-x.md" }))).toBe("Writing 00115-x.md")
    expect(phrase(tool("apply_patch", { path: "a/b.md" }))).toBe("Writing b.md")
    expect(phrase(tool("Bash", { command: "cargo test -p op-server" }))).toBe("Running cargo")
    expect(phrase(tool("shell", { command: ["ls", "-la"] }))).toBe("Running ls")
    expect(phrase(tool("TodoWrite", { todos: [] }))).toBe("Working")
  })

  it("names the daemon's binary by its name rather than its path", () => {
    expect(phrase(tool("Bash", { command: '/Users/x/.cargo/bin/openplan create "T" --tag ui' }))).toBe(
      "Running openplan",
    )
  })

  it("keeps the verb alone when the input names nothing", () => {
    expect(phrase(tool("Read", {}))).toBe("Reading")
    expect(phrase(tool("Bash", { command: "   " }))).toBe("Running")
    expect(phrase(tool("Bash", "not an object"))).toBe("Running")
  })
})

describe("chatItems", () => {
  it("folds a run of thinking and tools between two messages into one row", () => {
    const items = chatItems([
      { kind: "prompt", text: "Write a task" },
      message("On it."),
      thinking,
      tool("Read", { file_path: "/a/lib.rs" }),
      tool("Grep", {}),
      message("Done.", false),
    ])
    expect(items.map((item) => item.kind)).toEqual(["prompt", "message", "activity", "message"])
    const activity = items[2]
    if (activity.kind !== "activity") throw new Error("expected an activity row")
    expect(activity.steps).toHaveLength(3)
    expect(newest(activity.steps).phrase).toBe("Searching")
    expect(activity.steps.map((step) => step.phrase)).toEqual(["Thinking", "Reading lib.rs", "Searching"])
  })

  it("marks a failed step", () => {
    const items = chatItems([tool("Bash", { command: "cargo build" }, true), tool("Read", { file_path: "x.rs" })])
    expect(items).toEqual([
      {
        kind: "activity",
        steps: [
          { phrase: "Running cargo", done: true, failed: true },
          { phrase: "Reading x.rs", done: true, failed: false },
        ],
      },
    ])
  })

  it("closes the run when a message, a prompt, or a failure follows", () => {
    const items = chatItems([
      thinking,
      message("One."),
      thinking,
      { kind: "failure", message: "boom", retrying: true },
      thinking,
      { kind: "prompt", text: "Again" },
      thinking,
    ])
    expect(items.map((item) => item.kind)).toEqual([
      "activity",
      "message",
      "activity",
      "failure",
      "activity",
      "prompt",
      "activity",
    ])
  })
})

describe("latest", () => {
  const during = (entries: ReadonlyArray<Entry>, approvals: Transcript["approvals"] = []): Transcript => ({
    ...emptyTranscript,
    status: { kind: "running" },
    entries,
    approvals,
  })
  const prompt: Entry = { kind: "prompt", text: "do it" }

  it("says nothing before the first prompt, and thinks right after it", () => {
    expect(latest(emptyTranscript)).toEqual({ kind: "none" })
    expect(latest(during([prompt]))).toEqual({ kind: "step", phrase: "Thinking" })
  })

  it("names the newest step of a run, then the message that closes it", () => {
    expect(latest(during([prompt, thinking, tool("Read", { path: "a.md" }, false, false)]))).toEqual({
      kind: "step",
      phrase: "Reading a.md",
    })
    expect(latest(during([prompt, thinking, message("Done.", false)]))).toEqual({
      kind: "message",
      text: "Done.",
      done: false,
    })
  })

  it("puts a waiting approval before everything else", () => {
    const approval = { id: "ap1", item: null, tool: "Bash", input: { command: "rm x" }, reason: null }
    expect(latest(during([prompt, thinking], [approval]))).toEqual({ kind: "approval", count: 1 })
  })

  it("keeps the failure that ended the turn", () => {
    const failure: Entry = { kind: "failure", message: "rate limited", retrying: true }
    expect(latest({ ...emptyTranscript, entries: [prompt, failure] })).toEqual({
      kind: "failure",
      message: "rate limited",
    })
  })
})
