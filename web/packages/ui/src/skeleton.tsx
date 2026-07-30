import type * as React from "react"

import { cn } from "./cn"

export function Skeleton({ className, ...props }: React.ComponentProps<"div">) {
  return <div data-slot="skeleton" className={cn("bg-accent animate-pulse rounded-md", className)} {...props} />
}

export function SkeletonList({ count, className }: { count: number; className?: string }) {
  return (
    <div className="space-y-3">
      {Array.from({ length: count }, (_, i) => (
        <Skeleton key={i} className={className} />
      ))}
    </div>
  )
}
