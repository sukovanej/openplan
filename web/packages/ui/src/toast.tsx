import { X } from "lucide-react"
import type { ReactNode } from "react"

import { Button } from "./button"
import { cn } from "./cn"

const TONES = {
  ok: "bg-background text-foreground/90",
  danger: "border-danger-border/30 bg-danger-surface text-danger-foreground",
} as const

const SHAPES = {
  pill: "inline-block rounded-full px-3 py-1.5 text-xs font-medium",
  card: "flex items-start gap-3 rounded-lg px-4 py-3 text-sm",
} as const

export function Toast({
  tone = "ok",
  shape = "pill",
  role,
  live,
  onDismiss,
  className,
  children,
}: {
  tone?: keyof typeof TONES
  shape?: keyof typeof SHAPES
  role: string
  live?: "polite" | "assertive"
  onDismiss?: () => void
  className?: string
  children?: ReactNode
}) {
  // The region outlives any one message: a live region inserted together with its text is announced
  // unreliably, and an empty one is invisible anyway.
  return (
    <div role={role} aria-live={live} className={className}>
      {children !== undefined && children !== null && children !== false && (
        <div className={cn("border shadow-lg", TONES[tone], SHAPES[shape])}>
          <span className="min-w-0">{children}</span>
          {onDismiss !== undefined && (
            <Button variant="danger" aria-label="Dismiss" onClick={onDismiss} className="-mr-1 rounded p-0.5 text-sm">
              <X className="size-4" />
            </Button>
          )}
        </div>
      )}
    </div>
  )
}
