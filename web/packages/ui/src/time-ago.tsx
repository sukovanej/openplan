import { useSyncExternalStore } from "react"

import { absoluteTime, relativeTime } from "./time"
import { Tooltip } from "./tooltip"

const TICK_MS = 1000
const ticked = new Set<() => void>()
let clock: ReturnType<typeof setInterval> | undefined

function subscribe(listener: () => void): () => void {
  ticked.add(listener)
  clock ??= setInterval(() => ticked.forEach((one) => one()), TICK_MS)
  return () => {
    ticked.delete(listener)
    if (ticked.size > 0) return
    clearInterval(clock)
    clock = undefined
  }
}

// The snapshot is the text, so a tick re-renders only a time whose text changed.
function useRelativeTime(iso: string): string {
  return useSyncExternalStore(subscribe, () => relativeTime(iso))
}

// Relative text is what a reader wants at a glance; the exact instant stays one hover away rather
// than spending a column on it.
export function TimeAgo({ iso, label, className }: { iso: string; label: string; className?: string }) {
  const text = useRelativeTime(iso)
  return (
    <Tooltip content={`${label} ${absoluteTime(iso)}`}>
      <time dateTime={iso} className={className}>
        {text}
      </time>
    </Tooltip>
  )
}
