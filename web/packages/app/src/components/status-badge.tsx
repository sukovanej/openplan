import { Badge } from "@/components/ui/badge"
import type { Status } from "@/lib/api"

type Variant = "default" | "secondary" | "destructive" | "outline"

const styles: Record<Status, { label: string; variant: Variant; className?: string }> = {
  backlog: { label: "Backlog", variant: "outline" },
  todo: { label: "Todo", variant: "secondary" },
  in_progress: {
    label: "In progress",
    variant: "default",
    className: "border-transparent bg-blue-600 text-white",
  },
  done: {
    label: "Done",
    variant: "default",
    className: "border-transparent bg-emerald-600 text-white",
  },
  cancelled: { label: "Cancelled", variant: "outline", className: "text-muted-foreground" },
}

export function StatusBadge({ status }: { status: Status }) {
  const style = styles[status]
  return (
    <Badge variant={style.variant} className={style.className}>
      {style.label}
    </Badge>
  )
}
