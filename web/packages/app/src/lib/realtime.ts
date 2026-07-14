import { Schema } from "effect"

import { applyChange, ChangeEvent } from "./events"
import { storeInvalidator } from "./store"

const decode = Schema.decodeUnknownSync(ChangeEvent)

let started = false

// EventSource reconnects on its own with the browser's built-in backoff, so the feed stays
// live across daemon restarts without an explicit retry loop.
export function startRealtime(): void {
  if (started) return
  started = true
  const source = new EventSource("/api/events")
  source.onmessage = (message: MessageEvent<string>) => {
    let event: ChangeEvent
    try {
      event = decode(JSON.parse(message.data))
    } catch {
      return
    }
    applyChange(storeInvalidator, event)
  }
}
