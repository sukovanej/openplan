import { cn } from "./cn"

// Every standalone control is this tall, so controls side by side line up: a field, a select, a
// button beside a field, and a segmented group.
export const CONTROL_HEIGHT = "h-8"

export const SEGMENT_GROUP = `bg-muted inline-flex ${CONTROL_HEIGHT} items-center gap-0.5 rounded-md p-0.5`

// A segment fills the group inside its 2px padding.
export function segment(active: boolean): string {
  return cn(
    "focus-visible:ring-ring inline-flex h-7 items-center justify-center rounded-sm transition-colors focus-visible:ring-2 focus-visible:outline-none",
    active ? "bg-background text-foreground shadow-sm" : "text-muted-foreground hover:text-foreground",
  )
}
