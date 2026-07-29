import { absoluteTime, relativeTime } from "../lib/format"

// Relative text is what a reader wants at a glance; the exact instant stays one hover away rather
// than spending a column on it.
export function TimeAgo({ iso, label, className }: { iso: string; label: string; className?: string }) {
  return (
    <time dateTime={iso} title={`${label} ${absoluteTime(iso)}`} className={className}>
      {relativeTime(iso)}
    </time>
  )
}
