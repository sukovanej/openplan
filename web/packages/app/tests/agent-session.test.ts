import { describe, expect, it } from "vitest"

import { attachableSession, fold } from "../src/lib/agent-session"
import { emptyTranscript } from "../src/lib/agent-transcript"
import type { AgentSessionSummary } from "../src/lib/api"

const summary = (id: string, task: string | undefined, status: AgentSessionSummary["status"]): AgentSessionSummary => ({
  id,
  agent: "claude_code",
  task,
  status,
  started_at: "2026-09-10T00:00:00Z",
})

describe("attachableSession", () => {
  it("picks the newest live session bound to the task", () => {
    const sessions = [
      summary("newest", "OPP-2", { kind: "idle" }),
      summary("bound", "OPP-1", { kind: "running" }),
      summary("older", "OPP-1", { kind: "idle" }),
    ]
    expect(attachableSession(sessions, "OPP-1")).toBe("bound")
  })

  it("skips a session that has exited", () => {
    const sessions = [
      summary("gone", "OPP-1", { kind: "exited", code: 0 }),
      summary("live", "OPP-1", { kind: "starting" }),
    ]
    expect(attachableSession(sessions, "OPP-1")).toBe("live")
    expect(attachableSession([summary("gone", "OPP-1", { kind: "exited", code: null })], "OPP-1")).toBeUndefined()
  })

  it("picks none on the new-task route", () => {
    expect(attachableSession([summary("unbound", undefined, { kind: "idle" })], undefined)).toBeUndefined()
  })
})

describe("fold", () => {
  const snapshot = {
    kind: "snapshot" as const,
    id: "s1",
    project: "openplan",
    agent: "claude_code" as const,
    task: null,
    branch: "openplan/rolling-updates",
    cwd: "/w",
    status: { kind: "idle" as const },
    started_at: "2026-09-10T00:00:00Z",
    transcript: { ...emptyTranscript, status: { kind: "idle" as const } },
  }

  it("takes the snapshot as the view, then folds each event into its transcript", () => {
    const live = fold({ phase: "connecting" }, snapshot)
    if (live.phase !== "live") throw new Error("expected a live session")
    expect(live.view.id).toBe("s1")
    const next = fold(live, { kind: "agent", event: "turn_started", turn: "t1", prompt: "hi" })
    if (next.phase !== "live") throw new Error("expected a live session")
    expect(next.view.status).toEqual({ kind: "running" })
    expect(next.view.transcript.entries).toEqual([{ kind: "prompt", text: "hi" }])
  })

  it("binds the task the daemon names", () => {
    const live = fold({ phase: "connecting" }, snapshot)
    const bound = fold(live, { kind: "task", id: "OPP-9" })
    expect(bound.phase === "live" && bound.view.task).toBe("OPP-9")
  })

  it("ignores an event that arrives before the snapshot", () => {
    const early = fold({ phase: "connecting" }, { kind: "agent", event: "exited", code: 0 })
    expect(early).toEqual({ phase: "connecting" })
  })
})
