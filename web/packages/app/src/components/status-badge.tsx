import { CircleCheck, CircleDot, CircleEllipsis, CircleX, Clock, Eye, type LucideIcon } from "lucide-react"

import type { Status } from "@/lib/api"
import { cn } from "@/lib/utils"

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

export const statusOrder: ReadonlyArray<Status> = ["in_progress", "in_review", "todo", "backlog", "done", "cancelled"]

export const statusLabel = (status: Status): string => styles[status].label

export const statusHeaderClass = (status: Status): string => styles[status].palette.header

export function StatusIcon({ status }: { status: Status }) {
  const { icon: Icon, label, palette } = styles[status]
  return <Icon className={cn("size-5", palette.icon)} aria-label={label} />
}
