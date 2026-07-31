import { CalendarPlus, CircleAlert, CornerLeftUp, History, type LucideIcon } from "lucide-react"
import type { ReactNode } from "react"

import type { Field_Rfc3339 } from "@open-planner/api-client"

import { type FieldProblem, fieldFailure, fieldValue } from "../lib/metadata"
import { cn } from "../lib/utils"
import { TimeAgo } from "./time-ago"

export const PARENT_ICON = CornerLeftUp

// Items wrap rather than overrun each other once the line runs out of room, and each carries its own
// icon, so spacing alone separates them — a dot would strand itself at the head of a wrapped line.
export function MetaLine({ className, children }: { className?: string; children: ReactNode }) {
  return (
    <div
      className={cn(
        "text-muted-foreground flex min-w-0 flex-wrap items-center gap-x-3 gap-y-0.5 overflow-hidden text-xs",
        className,
      )}
    >
      {children}
    </div>
  )
}

export function MetaItem({
  icon: Icon,
  title,
  className,
  children,
}: {
  icon: LucideIcon
  title?: string
  className?: string
  children: ReactNode
}) {
  return (
    <span title={title} className={cn("flex min-w-0 items-center", className)}>
      <Icon className="mr-1 size-3.5 shrink-0 opacity-70" />
      {children}
    </span>
  )
}

export function TaskTimes({
  created,
  updated,
  problems,
}: {
  created: string | undefined
  updated: Field_Rfc3339 | undefined
  problems: ReadonlyArray<FieldProblem>
}) {
  const updatedAt = updated === undefined ? undefined : fieldValue(updated)
  // A commit git cannot date leaves the field carrying why instead of when, and that reads where the
  // time would have — not folded in with the frontmatter's own problems, which it is not one of.
  const undatable = updated === undefined ? undefined : fieldFailure(updated)
  return (
    <>
      {created !== undefined && (
        <MetaItem icon={CalendarPlus} className="shrink-0 whitespace-nowrap">
          <TimeAgo iso={created} label="Created" />
        </MetaItem>
      )}
      {updatedAt !== undefined && (
        <MetaItem icon={History} className="shrink-0 whitespace-nowrap">
          <TimeAgo iso={updatedAt} label="Updated" />
        </MetaItem>
      )}
      {undatable !== undefined && undatable.kind === "invalid" && (
        <MetaItem icon={History} title={undatable.message} className="min-w-0 text-red-600/90 dark:text-red-400/90">
          <span className="truncate">{undatable.message}</span>
        </MetaItem>
      )}
      {problems.length > 0 && (
        <MetaItem icon={CircleAlert} className="text-red-600/90 dark:text-red-400/90">
          <span className="truncate">{problems.map(({ field, message }) => `${field}: ${message}`).join(", ")}</span>
        </MetaItem>
      )}
    </>
  )
}
