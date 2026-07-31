import { CircleAlert, CircleCheck, CircleDot, CircleEllipsis, CircleX, Clock, Eye, type LucideIcon } from "lucide-react"

import type { Field_Status, Metadata, Status } from "@open-planner/api-client"
import { cn, Tooltip } from "@open-planner/ui"

import { fieldValue, statusField } from "./metadata"

const styles: Record<Status, { label: string; icon: LucideIcon; mark: string; header: string }> = {
  backlog: {
    label: "Backlog",
    icon: CircleEllipsis,
    mark: "text-status-backlog",
    header: "bg-status-backlog-surface/8 border-status-backlog-surface/20 text-status-backlog-text/80",
  },
  todo: {
    label: "Todo",
    icon: Clock,
    mark: "text-status-todo",
    header: "bg-status-todo-surface/8 border-status-todo-surface/20 text-status-todo-text/80",
  },
  in_progress: {
    label: "In progress",
    icon: CircleDot,
    mark: "text-status-in-progress",
    header: "bg-status-in-progress-surface/8 border-status-in-progress-surface/20 text-status-in-progress-text/80",
  },
  in_review: {
    label: "In review",
    icon: Eye,
    mark: "text-status-in-review",
    header: "bg-status-in-review-surface/8 border-status-in-review-surface/20 text-status-in-review-text/80",
  },
  done: {
    label: "Done",
    icon: CircleCheck,
    mark: "text-status-done",
    header: "bg-status-done-surface/8 border-status-done-surface/20 text-status-done-text/80",
  },
  cancelled: {
    label: "Cancelled",
    icon: CircleX,
    mark: "text-status-cancelled",
    header: "bg-status-cancelled-surface/8 border-status-cancelled-surface/20 text-status-cancelled-text/80",
  },
}

const UNREADABLE_HEADER =
  "bg-status-unreadable-surface/8 border-status-unreadable-surface/20 text-status-unreadable-text/80"

export const statusOrder: ReadonlyArray<Status> = ["in_review", "in_progress", "todo", "backlog", "done", "cancelled"]

export const statusLabel = (status: Status): string => styles[status].label

export function StatusIcon({ status, className }: { status: Status; className?: string }) {
  const { icon: Icon, label, mark } = styles[status]
  return (
    <Tooltip content={label}>
      <Icon className={cn("size-5", mark, className)} aria-label={label} />
    </Tooltip>
  )
}

const UNREADABLE = "Status could not be read"

function UnreadableStatus({ className }: { className?: string }) {
  return (
    <Tooltip content={UNREADABLE}>
      <CircleAlert className={cn("size-5 text-status-unreadable", className)} aria-label={UNREADABLE} />
    </Tooltip>
  )
}

// A status that could not be read gets its own mark, so a broken file never wears a status it does
// not claim.
export function StatusChip({ status, className }: { status: Field_Status | undefined; className?: string }) {
  const value = status === undefined ? undefined : fieldValue(status)
  return value === undefined ? (
    <UnreadableStatus className={className} />
  ) : (
    <StatusIcon status={value} className={className} />
  )
}

// Takes the whole metadata because a file whose frontmatter did not parse has no status field at all.
export function StatusField({ metadata, className }: { metadata: Metadata; className?: string }) {
  return <StatusChip status={statusField(metadata)} className={className} />
}

// A group with no status holds the tasks whose own status could not be read; it is not a status they
// claimed, so it gets its own label and tint rather than borrowing one.
export const statusGroupLabel = (status: Status | undefined): string =>
  status === undefined ? "Unreadable" : statusLabel(status)

export function StatusGroupHeader({ status }: { status: Status | undefined }) {
  return (
    <div role="row" className="bg-muted/20 border-b p-2">
      <span
        role="columnheader"
        className={cn(
          "inline-block rounded-md border px-2 py-0.5 text-xs font-medium tracking-wide uppercase",
          status === undefined ? UNREADABLE_HEADER : styles[status].header,
        )}
      >
        {statusGroupLabel(status)}
      </span>
    </div>
  )
}
