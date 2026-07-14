import { CircleCheck, CircleDot, CircleEllipsis, CircleX, Clock, Eye, type LucideIcon } from "lucide-react"

import { Badge } from "@/components/ui/badge"
import type { Status } from "@/lib/api"
import { cn } from "@/lib/utils"

type Variant = "default" | "secondary" | "destructive" | "outline"

const styles: Record<
  Status,
  { label: string; variant: Variant; icon: LucideIcon; iconClass: string; headerClass: string; className?: string }
> = {
  backlog: {
    label: "Backlog",
    variant: "outline",
    icon: CircleEllipsis,
    iconClass: "text-cyan-600",
    headerClass: "bg-cyan-500/8 border-cyan-500/20 text-cyan-700/80 dark:text-cyan-300/80",
  },
  todo: {
    label: "Todo",
    variant: "secondary",
    icon: Clock,
    iconClass: "text-amber-600",
    headerClass: "bg-amber-500/8 border-amber-500/20 text-amber-700/80 dark:text-amber-300/80",
  },
  in_progress: {
    label: "In progress",
    variant: "default",
    icon: CircleDot,
    iconClass: "text-blue-600",
    headerClass: "bg-blue-600/8 border-blue-600/20 text-blue-700/80 dark:text-blue-300/80",
    className: "border-transparent bg-blue-600 text-white",
  },
  in_review: {
    label: "In review",
    variant: "default",
    icon: Eye,
    iconClass: "text-violet-600",
    headerClass: "bg-violet-600/8 border-violet-600/20 text-violet-700/80 dark:text-violet-300/80",
    className: "border-transparent bg-violet-600 text-white",
  },
  done: {
    label: "Done",
    variant: "default",
    icon: CircleCheck,
    iconClass: "text-emerald-600",
    headerClass: "bg-emerald-600/8 border-emerald-600/20 text-emerald-700/80 dark:text-emerald-300/80",
    className: "border-transparent bg-emerald-600 text-white",
  },
  cancelled: {
    label: "Cancelled",
    variant: "outline",
    icon: CircleX,
    iconClass: "text-rose-600",
    headerClass: "bg-rose-500/8 border-rose-500/20 text-rose-700/80 dark:text-rose-300/80",
  },
}

export const statusOrder: ReadonlyArray<Status> = ["in_progress", "in_review", "todo", "backlog", "done", "cancelled"]

export const statusLabel = (status: Status): string => styles[status].label

export const statusHeaderClass = (status: Status): string => styles[status].headerClass

export function StatusBadge({ status }: { status: Status }) {
  const style = styles[status]
  return (
    <Badge variant={style.variant} className={style.className}>
      {style.label}
    </Badge>
  )
}

export function StatusIcon({ status }: { status: Status }) {
  const style = styles[status]
  const Icon = style.icon
  return <Icon className={cn("size-5", style.iconClass)} aria-label={style.label} />
}
