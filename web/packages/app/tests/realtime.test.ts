import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

const refreshed = vi.hoisted(() => ({ calls: [] as Array<string> }))

vi.mock("../src/lib/query-client", () => ({
  queryInvalidator: {
    refreshProjects: () => refreshed.calls.push("projects"),
    refreshList: (project: string) => refreshed.calls.push(`list ${project}`),
    refreshTask: (project: string, id: string) => refreshed.calls.push(`task ${project}/${id}`),
    refreshHistory: (project: string) => refreshed.calls.push(`history ${project}`),
    refreshSync: (project: string) => refreshed.calls.push(`sync ${project}`),
    refreshVisible: (project?: string) => refreshed.calls.push(`screen ${project ?? "every"}`),
  },
}))

class FakeEventSource {
  static made: Array<FakeEventSource> = []
  onopen: (() => void) | null = null
  onmessage: ((message: MessageEvent<string>) => void) | null = null
  onerror: (() => void) | null = null

  constructor(readonly url: string) {
    FakeEventSource.made.push(this)
  }

  close(): void {}

  send(event: object, id = ""): void {
    this.onmessage?.({ data: JSON.stringify(event), lastEventId: id } as MessageEvent<string>)
  }
}

beforeEach(() => {
  vi.useFakeTimers()
  vi.stubGlobal("EventSource", FakeEventSource)
  // The stream is a module singleton, so each test starts a fresh module.
  vi.resetModules()
  FakeEventSource.made = []
  refreshed.calls = []
})

afterEach(() => {
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

async function start(): Promise<FakeEventSource> {
  const { startRealtime } = await import("../src/lib/realtime")
  startRealtime()
  const stream = FakeEventSource.made[0]
  stream.onopen?.()
  return stream
}

function reconnect(stream: FakeEventSource): FakeEventSource {
  stream.onerror?.()
  vi.advanceTimersByTime(1000)
  const next = FakeEventSource.made.at(-1)!
  next.onopen?.()
  return next
}

describe("the event stream", () => {
  // A change can come between the first reads and the open, and no stream carries it.
  it("reads the projects and everything on screen again when the first stream opens", async () => {
    const stream = await start()
    expect(stream.url).toBe("/api/events")
    expect(refreshed.calls).toEqual(["projects", "screen every"])
  })

  it("refreshes each read once for a burst of events, after a short wait", async () => {
    const stream = await start()
    refreshed.calls = []

    for (const id of ["OPP-1", "OPP-2", "OPP-3"]) stream.send({ kind: "task_changed", project: "openplan", id })
    expect(refreshed.calls).toEqual([])

    vi.advanceTimersByTime(50)
    expect(refreshed.calls).toEqual([
      "list openplan",
      "task openplan/OPP-1",
      "task openplan/OPP-2",
      "task openplan/OPP-3",
      "history openplan",
    ])
  })

  // The daemon replays what came after the cursor, or sends a resync when it cannot.
  it("resumes from the last event it saw, and reads nothing again when it opens", async () => {
    const stream = await start()
    stream.send({ kind: "sync_changed", project: "openplan" }, "7")
    vi.advanceTimersByTime(50)
    refreshed.calls = []

    const next = reconnect(stream)
    vi.advanceTimersByTime(50)

    expect(next.url).toBe("/api/events?last_event_id=7")
    expect(refreshed.calls).toEqual([])
  })

  it("reads everything again when it reconnects before any event came", async () => {
    const stream = await start()
    refreshed.calls = []

    const next = reconnect(stream)

    expect(next.url).toBe("/api/events")
    expect(refreshed.calls).toEqual(["projects", "screen every"])
  })
})
