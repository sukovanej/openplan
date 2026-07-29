import { CalendarPlus, CircleAlert, CornerLeftUp, History, type LucideIcon } from "lucide-react"
import type { ReactNode } from "react"

import { TimeAgo } from "@/components/time-ago"
import type { FieldProblem } from "@/lib/metadata"
import { cn } from "@/lib/utils"

export const PARENT_ICON = CornerLeftUp

// Separators come from the items themselves, so any subset of them can be absent without leaving a
// stray dot behind.
export function MetaLine({ className, children }: { className?: string; children: ReactNode }) {
  return (
    <div
      className={cn(
        "text-muted-foreground flex min-w-0 items-center text-xs",
        "[&>*+*]:before:mx-1.5 [&>*+*]:before:text-muted-foreground/50 [&>*+*]:before:content-['·']",
        className,
      )}
    >
      {children}
    </div>
  )
}

// The icon keeps its distance with a margin rather than the item's flex `gap`: the separator is a
// pseudo-element inside the item that follows, and a gap would land on one of its sides only.
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
  updated: string | undefined
  problems: ReadonlyArray<FieldProblem>
}) {
  return (
    <>
      {created !== undefined && (
        <MetaItem icon={CalendarPlus} className="whitespace-nowrap">
          <TimeAgo iso={created} label="Created" />
        </MetaItem>
      )}
      {updated !== undefined && (
        <MetaItem icon={History} className="whitespace-nowrap">
          <TimeAgo iso={updated} label="Updated" />
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
