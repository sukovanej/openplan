import type { ReactNode } from "react"

import type { Field_Status } from "@open-planner/api-client"
import { cn, FuzzyText } from "@open-planner/ui"

import { StatusChip } from "./status"

// The same status mark, key and title everywhere a task names itself; only the two rungs — how big
// the mark is and how small the key reads — differ between the surfaces that show it.
const VARIANTS = {
  row: { gap: "gap-2", mark: "size-4", id: "text-xs" },
  header: { gap: "gap-2", mark: "size-5", id: "text-xs" },
  chip: { gap: "gap-1", mark: "size-4", id: "" },
} as const

export function TaskIdentity({
  status,
  mark,
  id,
  title,
  indices,
  variant = "row",
  className,
}: {
  // Stated even when there is none, so a caller cannot omit it and have the task wear the mark of a
  // status that could not be read.
  status: Field_Status | undefined
  // A reference the store could not resolve leads with a mark of its own rather than a status it has
  // no way to know.
  mark?: ReactNode
  id: string
  title?: string
  indices?: ReadonlyArray<number>
  variant?: keyof typeof VARIANTS
  className?: string
}) {
  const style = VARIANTS[variant]
  return (
    <span className={cn("flex min-w-0 items-center", style.gap, className)}>
      {mark ?? <StatusChip status={status} className={cn(style.mark, "shrink-0")} />}
      <span className={cn("text-muted-foreground shrink-0 tabular-nums", style.id)}>{id}</span>
      {title !== undefined && (
        <span className="min-w-0 truncate">
          {indices === undefined ? title : <FuzzyText text={title} indices={indices} />}
        </span>
      )}
    </span>
  )
}
