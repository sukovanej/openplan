import { Tooltip } from "@open-planner/ui"

// Focusable, so the keyboard reaches the reason too: it replaces a control that had focus of its own.
export function Blocked({ reason }: { reason: string }) {
  return (
    <Tooltip content={reason}>
      <span tabIndex={0} className="text-muted-foreground/70 text-xs italic">
        read-only
      </span>
    </Tooltip>
  )
}
