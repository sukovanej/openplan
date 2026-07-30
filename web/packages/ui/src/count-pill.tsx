import { cn } from "./cn"

export function CountPill({ count, className }: { count: number; className?: string }) {
  return (
    <span className={cn("bg-muted text-muted-foreground rounded-full px-1.5 text-[11px] tabular-nums", className)}>
      {count}
    </span>
  )
}
