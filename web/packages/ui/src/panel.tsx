import type * as React from "react"

import { cn } from "./cn"

export function Panel({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      // The outline is an inset ring rather than a border, so a current row's own ring lands on the
      // same pixels and reads as one line instead of doubling up against the frame. It lies over the
      // content, because Chrome on Windows paints a scrollbar over anything under it. The surface is a
      // solid mix rather than a see-through tint, so a diagram's label boxes can paint the same color.
      className={cn(
        "bg-(--surface) [--surface:color-mix(in_srgb,var(--muted)_10%,var(--background))] relative flex h-full flex-col overflow-hidden rounded-lg after:pointer-events-none after:absolute after:inset-0 after:rounded-[inherit] after:ring-1 after:ring-inset after:ring-border after:content-['']",
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
    <div
      className={cn("text-muted-foreground min-w-0 text-xs font-medium tracking-wide uppercase", className)}
      {...props}
    />
  )
}

export function PanelBody({ className, ...props }: React.ComponentProps<"div">) {
  return <div className={cn("min-h-0 flex-1 overflow-y-auto", className)} {...props} />
}
