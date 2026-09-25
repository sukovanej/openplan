import { Schema } from "effect"

import { connectionStore } from "./connection"
import { applyChange, ChangeEvent, coalesced } from "./events"
import { queryInvalidator } from "./query-client"

const decode = Schema.decodeUnknownSync(ChangeEvent)

const RECONNECT_BASE_MS = 1000
const RECONNECT_CAP_MS = 8000
const STOPPED_POLL_MS = 10000
// The events of one change arrive back to back, well inside this.
const COALESCE_MS = 50

const invalidator = coalesced(queryInvalidator, (flush) => {
  setTimeout(flush, COALESCE_MS)
})

let started = false
let source: EventSource | null = null
let timer: ReturnType<typeof setTimeout> | undefined
let attempts = 0
let stopped = false
let lastEventId: string | undefined

// A new EventSource cannot set the Last-Event-ID header, so the cursor goes in the query instead.
function eventsUrl(cursor: string | undefined): string {
  return cursor === undefined ? "/api/events" : `/api/events?${new URLSearchParams({ last_event_id: cursor })}`
}

function connect(): void {
  const cursor = lastEventId
  source = new EventSource(eventsUrl(cursor))

  source.onopen = () => {
    attempts = 0
    stopped = false
    connectionStore.set("live")
    // A stream that resumes from a cursor gets what it missed replayed, or a resync when the daemon
    // cannot replay it. A stream without one can have missed a change between the first reads and
    // the open, so it reads the projects and everything on screen again.
    if (cursor === undefined) {
      queryInvalidator.refreshProjects()
      queryInvalidator.refreshVisible()
    }
  }

  source.onmessage = (message: MessageEvent<string>) => {
    if (message.lastEventId !== "") lastEventId = message.lastEventId
    let event: ChangeEvent
    try {
      event = decode(JSON.parse(message.data))
    } catch {
      return
    }
    if (event.kind === "daemon_stopping") {
      stopped = true
      connectionStore.set("stopped")
      return
    }
    applyChange(invalidator, event)
  }

  // EventSource would silently auto-reconnect on its own schedule; close it so status and backoff
  // stay observable and under our control.
  source.onerror = () => {
    source?.close()
    source = null
    scheduleReconnect()
  }
}

function scheduleReconnect(): void {
  if (timer !== undefined) return
  if (stopped) {
    // A clean stop: poll slowly until the daemon comes back rather than hammering it.
    connectionStore.set("stopped")
    timer = setTimeout(reconnect, STOPPED_POLL_MS)
    return
  }
  connectionStore.set("reconnecting")
  const delay = Math.min(RECONNECT_BASE_MS * 2 ** attempts, RECONNECT_CAP_MS)
  attempts += 1
  timer = setTimeout(reconnect, delay)
}

function reconnect(): void {
  timer = undefined
  connect()
}

export function startRealtime(): void {
  if (started) return
  started = true
  connect()
}
