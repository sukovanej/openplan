import { absoluteTime, relativeTime } from "./time"
import { Tooltip } from "./tooltip"

// Relative text is what a reader wants at a glance; the exact instant stays one hover away rather
// than spending a column on it.
export function TimeAgo({ iso, label, className }: { iso: string; label: string; className?: string }) {
  return (
    <Tooltip content={`${label} ${absoluteTime(iso)}`}>
      <time dateTime={iso} className={className}>
        {relativeTime(iso)}
      </time>
    </Tooltip>
  )
}
