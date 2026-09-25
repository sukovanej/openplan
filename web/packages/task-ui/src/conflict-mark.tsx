import { TriangleAlert } from "lucide-react"

import { cn, Tooltip } from "@openplan/ui"

export const CONFLICT_TINT = "border-warning/40 bg-warning/10 text-warning"

// Borders included, a badge is as tall as the 16px meta line it sits in.
export const META_BADGE =
  "inline-flex shrink-0 items-center gap-1 rounded-md border px-1.5 text-[11px] leading-[14px] font-medium whitespace-nowrap"

export const CONFLICT_BADGE = cn(META_BADGE, CONFLICT_TINT)

export const conflictCount = (count: number): string => `${count} ${count === 1 ? "conflict" : "conflicts"}`

export function ConflictBadge({ count, className }: { count: number; className?: string }) {
  return (
    <Tooltip content={`${conflictCount(count)} from a sync. Open the task to pick a version of each.`}>
      <span tabIndex={0} className={cn(CONFLICT_BADGE, className)}>
        <TriangleAlert aria-hidden className="size-3 shrink-0" />
        {conflictCount(count)}
      </span>
    </Tooltip>
  )
}

// Until someone picks, readers see the version the remote published.
export function InForceMark({ className }: { className?: string }) {
  return (
    <span
      className={cn(
        "border-info/40 text-info rounded-sm border px-1 text-[10px] leading-[14px] font-medium tracking-wide uppercase",
        className,
      )}
    >
      in force
    </span>
  )
}
