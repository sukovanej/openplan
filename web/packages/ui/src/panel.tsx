import type * as React from "react"

import { cn } from "./cn"

export function Panel({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      // The outline is an inset ring rather than a border, so a current row's own ring lands on the
      // same pixels and reads as one line instead of doubling up against the frame.
      className={cn(
        "bg-muted/10 flex h-full flex-col overflow-hidden rounded-lg ring-1 ring-inset ring-border",
        className,
      )}
      {...props}
    />
  )
}

export function PanelHeader({ className, ...props }: React.ComponentProps<"div">) {
  return <div className={cn("bg-muted/30 flex h-11 shrink-0 items-center border-b px-4", className)} {...props} />
}

export function PanelTitle({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div className={cn("text-muted-foreground text-xs font-medium tracking-wide uppercase", className)} {...props} />
  )
}

export function PanelBody({ className, ...props }: React.ComponentProps<"div">) {
  return <div className={cn("min-h-0 flex-1 overflow-y-auto", className)} {...props} />
}
