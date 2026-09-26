import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

const refreshed = vi.hoisted(() => ({ calls: [] as Array<string> }))
const page = vi.hoisted(() => ({ unsaved: false, reloads: 0, version: "1.0.0" as string | undefined, up: true }))

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

vi.mock("../src/lib/unsaved-work", () => ({
  holdsUnsavedWork: () => page.unsaved,
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

// `/health` answers with the daemon's version, the text "ok" from a server that is not a daemon,
// or nothing while the daemon is down.
async function health(): Promise<Response> {
  if (!page.up) throw new TypeError("connection refused")
  if (page.version === undefined) return new Response("ok")
  return new Response(JSON.stringify({ pid: 1, port: 7373, started_at: 0, version: page.version }))
}

beforeEach(() => {
  vi.useFakeTimers()
  vi.stubGlobal("EventSource", FakeEventSource)
  vi.stubGlobal("fetch", health)
  vi.stubGlobal("document", {})
  vi.stubGlobal("window", { location: { reload: () => (page.reloads += 1) } })
  // The stream is a module singleton, so each test starts a fresh module.
  vi.resetModules()
  FakeEventSource.made = []
  refreshed.calls = []
  Object.assign(page, { unsaved: false, reloads: 0, version: "1.0.0", up: true })
})

afterEach(() => {
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

async function start(): Promise<FakeEventSource> {
  const { startRealtime } = await import("../src/lib/realtime")
  startRealtime()
  await vi.advanceTimersByTimeAsync(0)
  const stream = FakeEventSource.made[0]
  stream.onopen?.()
  return stream
}

async function connection(): Promise<string> {
  const { connectionStore } = await import("../src/lib/connection")
  return connectionStore.getSnapshot()
}

async function drop(stream: FakeEventSource, wait = 1000): Promise<FakeEventSource | undefined> {
  const before = FakeEventSource.made.length
  stream.onerror?.()
  await vi.advanceTimersByTimeAsync(wait)
  return FakeEventSource.made.length > before ? FakeEventSource.made.at(-1) : undefined
}

async function reconnect(stream: FakeEventSource): Promise<FakeEventSource> {
  const next = await drop(stream)
  next!.onopen?.()
  return next!
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

    const next = await reconnect(stream)
    vi.advanceTimersByTime(50)

    expect(next.url).toBe("/api/events?last_event_id=7")
    expect(refreshed.calls).toEqual([])
  })

  it("reads everything again when it reconnects before any event came", async () => {
    const stream = await start()
    refreshed.calls = []

    const next = await reconnect(stream)

    expect(next.url).toBe("/api/events")
    expect(refreshed.calls).toEqual(["projects", "screen every"])
  })

  it("waits ten seconds before it connects again after a stop", async () => {
    const stream = await start()
    stream.send({ kind: "daemon_stopping", reason: "stop" })
    expect(await connection()).toBe("stopped")

    expect(await drop(stream)).toBeUndefined()
    await vi.advanceTimersByTimeAsync(9000)
    expect(FakeEventSource.made).toHaveLength(2)
  })

  it("connects again at once after a stop for an update", async () => {
    const stream = await start()
    stream.send({ kind: "daemon_stopping", reason: "update" })
    expect(await connection()).toBe("updating")

    expect(await drop(stream)).toBeDefined()
  })

  it("keeps saying it updates while the new daemon starts", async () => {
    const stream = await start()
    stream.send({ kind: "daemon_stopping", reason: "update" })
    page.up = false

    expect(await drop(stream)).toBeUndefined()
    expect(await connection()).toBe("updating")
  })
})

describe("a new version of the daemon", () => {
  it("reloads the page before the stream opens again", async () => {
    const stream = await start()
    page.version = "1.1.0"

    expect(await drop(stream)).toBeUndefined()
    expect(page.reloads).toBe(1)
    expect(refreshed.calls).toEqual(["projects", "screen every"])
  })

  it("does not reload the page for the same version", async () => {
    const stream = await start()

    expect(await drop(stream)).toBeDefined()
    expect(page.reloads).toBe(0)
  })

  it("does not reload the page when the server tells no version", async () => {
    page.version = undefined
    const stream = await start()

    expect(await drop(stream)).toBeDefined()
    expect(page.reloads).toBe(0)
  })

  it("asks for a reload when the page holds unsaved work", async () => {
    const stream = await start()
    page.version = "1.1.0"
    page.unsaved = true

    expect(await drop(stream)).toBeUndefined()
    expect(page.reloads).toBe(0)
    expect(await connection()).toBe("outdated")
  })
})
