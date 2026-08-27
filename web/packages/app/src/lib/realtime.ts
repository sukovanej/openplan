import { Schema } from "effect"

import { connectionStore } from "./connection"
import { applyChange, ChangeEvent } from "./events"
import { queryInvalidator } from "./query-client"

const decode = Schema.decodeUnknownSync(ChangeEvent)

const RECONNECT_BASE_MS = 1000
const RECONNECT_CAP_MS = 8000
const STOPPED_POLL_MS = 10000

let started = false
let source: EventSource | null = null
let timer: ReturnType<typeof setTimeout> | undefined
let attempts = 0
let stopped = false

function connect(): void {
  source = new EventSource("/api/events")

  source.onopen = () => {
    attempts = 0
    stopped = false
    connectionStore.set("live")
    // Recover anything missed while disconnected: the projects that spell every id, then everything
    // currently on screen.
    queryInvalidator.refreshProjects()
    queryInvalidator.refreshVisible()
  }

  source.onmessage = (message: MessageEvent<string>) => {
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
    applyChange(queryInvalidator, event)
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
