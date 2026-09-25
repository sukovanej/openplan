import { describe, expect, it } from "vitest"

import { fold } from "../src/lib/agent-session"
import { emptyTranscript } from "../src/lib/agent-transcript"

describe("fold", () => {
  const snapshot = {
    kind: "snapshot" as const,
    id: "s1",
    project: "openplan",
    agent: "claude_code" as const,
    title: "why is it blocked",
    context: "OPP-3",
    tasks: [],
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

  it("takes the tasks the daemon says the session wrote, and keeps its context", () => {
    const live = fold({ phase: "connecting" }, snapshot)
    const wrote = fold(live, { kind: "tasks", tasks: ["OPP-9", "OPP-3"] })
    if (wrote.phase !== "live") throw new Error("expected a live session")
    expect(wrote.view.tasks).toEqual(["OPP-9", "OPP-3"])
    expect(wrote.view.context).toBe("OPP-3")
  })

  it("ignores an event that arrives before the snapshot", () => {
    const early = fold({ phase: "connecting" }, { kind: "agent", event: "exited", code: 0 })
    expect(early).toEqual({ phase: "connecting" })
  })
})
