import type { ReactNode } from "react"

import { cn } from "./cn"

// The hue is the caller's — `className` carries the border and text colour that say what the tag is
// about.
export function Tag({
  className,
  dashed = false,
  selected = false,
  onSelect,
  children,
}: {
  className?: string
  dashed?: boolean
  selected?: boolean
  onSelect?: () => void
  children: ReactNode
}) {
  const interactive = onSelect !== undefined
  const classes = cn(
    "inline-flex items-center gap-1 rounded-md border px-2 py-1 font-mono text-[11px] leading-tight",
    "whitespace-nowrap transition-opacity",
    className,
    dashed && "border-dashed",
    interactive && "cursor-pointer",
    interactive && !selected && "opacity-55 hover:opacity-100",
    interactive && selected && "font-semibold",
  )
  return onSelect === undefined ? (
    <span className={classes}>{children}</span>
  ) : (
    <button type="button" onClick={onSelect} aria-pressed={selected} className={classes}>
      {children}
    </button>
  )
}
