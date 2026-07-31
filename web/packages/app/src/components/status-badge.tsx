import { CircleAlert, CircleCheck, CircleDot, CircleEllipsis, CircleX, Clock, Eye, type LucideIcon } from "lucide-react"

import type { Field_Status, Metadata, Status } from "@open-planner/api-client"
import { cn, Tooltip } from "@open-planner/ui"

import { fieldValue, statusOf } from "../lib/metadata"

interface Palette {
  readonly icon: string
  readonly header: string
}

const gray: Palette = {
  icon: "text-gray-500",
  header: "bg-gray-500/8 border-gray-500/20 text-gray-600/80 dark:text-gray-400/80",
}

const blue: Palette = {
  icon: "text-blue-600",
  header: "bg-blue-500/8 border-blue-500/20 text-blue-700/80 dark:text-blue-300/80",
}

const amber: Palette = {
  icon: "text-amber-600",
  header: "bg-amber-600/8 border-amber-600/20 text-amber-700/80 dark:text-amber-300/80",
}

const red: Palette = {
  icon: "text-red-500",
  header: "bg-red-500/8 border-red-500/20 text-red-600/80 dark:text-red-300/80",
}

const emerald: Palette = {
  icon: "text-emerald-600",
  header: "bg-emerald-600/8 border-emerald-600/20 text-emerald-700/80 dark:text-emerald-300/80",
}

const styles: Record<Status, { label: string; icon: LucideIcon; palette: Palette }> = {
  backlog: { label: "Backlog", icon: CircleEllipsis, palette: gray },
  todo: { label: "Todo", icon: Clock, palette: blue },
  in_progress: { label: "In progress", icon: CircleDot, palette: amber },
  in_review: { label: "In review", icon: Eye, palette: red },
  done: { label: "Done", icon: CircleCheck, palette: emerald },
  cancelled: { label: "Cancelled", icon: CircleX, palette: gray },
}

export const statusOrder: ReadonlyArray<Status> = ["in_review", "in_progress", "todo", "backlog", "done", "cancelled"]

export const statusLabel = (status: Status): string => styles[status].label

export const statusHeaderClass = (status: Status): string => styles[status].palette.header

const UNREADABLE = "Status could not be read"

// A single status field, for a chip that has one but no surrounding metadata.
export function StatusChip({ status, className }: { status: Field_Status; className?: string }) {
  const value = fieldValue(status)
  return value === undefined ? (
    <UnreadableStatus className={className} />
  ) : (
    <StatusIcon status={value} className={className} />
  )
}

export function StatusIcon({ status, className }: { status: Status; className?: string }) {
  const { icon: Icon, label, palette } = styles[status]
  return (
    <Tooltip content={label}>
      <Icon className={cn("size-5", palette.icon, className)} aria-label={label} />
    </Tooltip>
  )
}

// A status that could not be read gets its own mark, so a broken file never wears a status it does
// not claim. Takes the whole metadata because a file whose frontmatter did not parse has no status
// field at all.
export function StatusField({ metadata, className }: { metadata: Metadata; className?: string }) {
  const value = statusOf(metadata)
  return value === undefined ? (
    <UnreadableStatus className={className} />
  ) : (
    <StatusIcon status={value} className={className} />
  )
}

function UnreadableStatus({ className }: { className?: string }) {
  return (
    <Tooltip content={UNREADABLE}>
      <CircleAlert className={cn("size-5 text-red-500", className)} aria-label={UNREADABLE} />
    </Tooltip>
  )
}
