import { TriangleAlert } from "lucide-react"
import { type ReactNode, useCallback, useRef, useState } from "react"

import { Button, cn, Popover } from "@openplan/ui"

import { CONFLICT_BADGE, InForceMark } from "./conflict-mark"

// One version of a field, in the order the conflict lists them; the last is the one in force. A
// version without `keep` cannot be written back, so it offers no button.
export interface ConflictChoice {
  readonly label: string
  readonly value: ReactNode
  readonly keep?: () => void
}

export function FieldConflict({
  field,
  choices,
  pending = false,
  trigger = "Conflict",
  align = "start",
  className,
}: {
  field: string
  choices: ReadonlyArray<ConflictChoice>
  pending?: boolean
  trigger?: ReactNode
  align?: "start" | "end"
  className?: string
}) {
  const [open, setOpen] = useState(false)
  const anchor = useRef<HTMLButtonElement>(null)
  const close = useCallback(() => setOpen(false), [])
  return (
    <span className={cn("inline-flex", className)}>
      <button
        ref={anchor}
        type="button"
        aria-haspopup="dialog"
        aria-expanded={open}
        aria-label={`${field}: two versions from a sync`}
        onClick={() => setOpen(!open)}
        className={cn(
          CONFLICT_BADGE,
          "hover:bg-warning/20 focus-visible:ring-ring cursor-pointer transition-colors focus-visible:ring-2 focus-visible:outline-none",
        )}
      >
        <TriangleAlert aria-hidden className="size-3 shrink-0" />
        {trigger}
      </button>
      {open && (
        <Popover anchor={anchor} label={`${field} conflict`} align={align} onClose={close} className="w-80">
          <p className="text-muted-foreground mb-1 text-xs">
            A sync kept two versions of this field. The last version is in force until you pick one.
          </p>
          <ul className="divide-y">
            {choices.map((choice, at) => (
              <li key={at} className="flex flex-col items-start gap-1.5 py-2.5 last:pb-0">
                <span className="flex w-full min-w-0 items-center gap-2">
                  <span className="text-muted-foreground min-w-0 truncate font-mono text-[11px]">{choice.label}</span>
                  {at === choices.length - 1 && <InForceMark />}
                </span>
                <div className="min-w-0">{choice.value}</div>
                {choice.keep !== undefined && (
                  <Button
                    variant="accent"
                    disabled={pending}
                    onClick={() => {
                      setOpen(false)
                      choice.keep?.()
                    }}
                    className="-ml-2"
                  >
                    Keep {choice.label}
                  </Button>
                )}
              </li>
            ))}
          </ul>
        </Popover>
      )}
    </span>
  )
}
