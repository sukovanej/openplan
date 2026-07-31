import type { LucideIcon } from "lucide-react"
import type { ReactNode } from "react"

import { cn } from "./cn"

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
  className,
  children,
}: {
  icon: LucideIcon
  className?: string
  children: ReactNode
}) {
  return (
    <span className={cn("flex min-w-0 items-center", className)}>
      <Icon className="mr-1 size-3.5 shrink-0 opacity-70" />
      {children}
    </span>
  )
}
