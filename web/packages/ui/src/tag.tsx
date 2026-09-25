import type { ReactNode } from "react"

import { cn } from "./cn"

// The hue is the caller's — `className` carries the border and text colour that say what the tag is
// about.
export function Tag({
  className,
  dashed = false,
  children,
}: {
  className?: string
  dashed?: boolean
  children: ReactNode
}) {
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1 rounded-md border px-2 py-1 font-mono text-[11px] leading-tight whitespace-nowrap",
        className,
        dashed && "border-dashed",
      )}
    >
      {children}
    </span>
  )
}
