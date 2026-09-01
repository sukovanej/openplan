import { CircleAlert, CircleCheck, CircleDot, CircleEllipsis, CircleX, Clock, Eye, type LucideIcon } from "lucide-react"

import type { Field_Status, Metadata, Status } from "@open-planner/api-client"
import { cn, Tooltip } from "@open-planner/ui"

import { fieldValue, statusField } from "./metadata"

const styles: Record<
  Status,
  { label: string; icon: LucideIcon; mark: string; header: string; border: string; surface: string; faint: string }
> = {
  backlog: {
    label: "Backlog",
    icon: CircleEllipsis,
    mark: "text-status-backlog",
    header: "bg-status-backlog-surface/8 border-status-backlog-surface/20 text-status-backlog-text/80",
    border: "border-status-backlog",
    surface: "bg-status-backlog-surface/12",
    faint: "bg-status-backlog-surface/6",
  },
  todo: {
    label: "Todo",
    icon: Clock,
    mark: "text-status-todo",
    header: "bg-status-todo-surface/8 border-status-todo-surface/20 text-status-todo-text/80",
    border: "border-status-todo",
    surface: "bg-status-todo-surface/12",
    faint: "bg-status-todo-surface/6",
  },
  in_progress: {
    label: "In progress",
    icon: CircleDot,
    mark: "text-status-in-progress",
    header: "bg-status-in-progress-surface/8 border-status-in-progress-surface/20 text-status-in-progress-text/80",
    border: "border-status-in-progress",
    surface: "bg-status-in-progress-surface/12",
    faint: "bg-status-in-progress-surface/6",
  },
  in_review: {
    label: "In review",
    icon: Eye,
    mark: "text-status-in-review",
    header: "bg-status-in-review-surface/8 border-status-in-review-surface/20 text-status-in-review-text/80",
    border: "border-status-in-review",
    surface: "bg-status-in-review-surface/12",
    faint: "bg-status-in-review-surface/6",
  },
  done: {
    label: "Done",
    icon: CircleCheck,
    mark: "text-status-done",
    header: "bg-status-done-surface/8 border-status-done-surface/20 text-status-done-text/80",
    border: "border-status-done",
    surface: "bg-status-done-surface/12",
    faint: "bg-status-done-surface/6",
  },
  cancelled: {
    label: "Cancelled",
    icon: CircleX,
    mark: "text-status-cancelled",
    header: "bg-status-cancelled-surface/8 border-status-cancelled-surface/20 text-status-cancelled-text/80",
    border: "border-status-cancelled",
    surface: "bg-status-cancelled-surface/12",
    faint: "bg-status-cancelled-surface/6",
  },
}

const UNREADABLE_HEADER =
  "bg-status-unreadable-surface/8 border-status-unreadable-surface/20 text-status-unreadable-text/80"
const UNREADABLE_BORDER = "border-status-unreadable"
const UNREADABLE_SURFACE = "bg-status-unreadable-surface/12"
const UNREADABLE_FAINT = "bg-status-unreadable-surface/6"

export const statusLabel = (status: Status): string => styles[status].label

// The status as the colour of a whole frame. A status that could not be read wears the unreadable
// mark's colour, as the icon does.
export const statusBorder = (status: Status | undefined): string =>
  status === undefined ? UNREADABLE_BORDER : styles[status].border

// The same colour as a wash behind a whole surface. `statusFaint` is the one a box wears, so a card
// that lies on it still reads as the nearer thing.
export const statusSurface = (status: Status | undefined): string =>
  status === undefined ? UNREADABLE_SURFACE : styles[status].surface

export const statusFaint = (status: Status | undefined): string =>
  status === undefined ? UNREADABLE_FAINT : styles[status].faint

export function StatusIcon({ status, className }: { status: Status; className?: string }) {
  const { icon: Icon, label, mark } = styles[status]
  return (
    <Tooltip content={label}>
      <Icon className={cn("size-5", mark, className)} aria-label={label} />
    </Tooltip>
  )
}

const UNREADABLE = "Status could not be read"

// A status that could not be read gets its own mark, so a broken file never wears a status it does
// not claim.
function markOf(status: Field_Status | undefined): { label: string; icon: LucideIcon; tint: string } {
  const value = status === undefined ? undefined : fieldValue(status)
  if (value === undefined) return { label: UNREADABLE, icon: CircleAlert, tint: "text-status-unreadable" }
  const { label, icon, mark } = styles[value]
  return { label, icon, tint: mark }
}

// The mark with no tooltip over it. The tooltip hangs off the viewport, which a transformed ancestor
// (the flow diagram's) makes it miss, so a surface that transforms takes this one.
export function StatusMark({ status, className }: { status: Field_Status | undefined; className?: string }) {
  const { icon: Icon, label, tint } = markOf(status)
  return <Icon className={cn("size-5", tint, className)} aria-label={label} />
}

export function StatusChip({ status, className }: { status: Field_Status | undefined; className?: string }) {
  return (
    <Tooltip content={markOf(status).label}>
      <StatusMark status={status} className={className} />
    </Tooltip>
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
