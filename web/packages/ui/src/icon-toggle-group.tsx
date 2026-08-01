import type { LucideIcon } from "lucide-react"
import type * as React from "react"

import { cn } from "./cn"
import { Tooltip } from "./tooltip"

export interface IconToggleOption<T extends string> {
  readonly value: T
  readonly label: string
  readonly Icon: LucideIcon
}

export function IconToggleGroup<T extends string>({
  label,
  options,
  value,
  onChange,
  className,
  ...props
}: {
  label: string
  options: ReadonlyArray<IconToggleOption<T>>
  value: T
  onChange: (value: T) => void
} & Omit<React.ComponentProps<"div">, "onChange">) {
  return (
    <div
      role="radiogroup"
      aria-label={label}
      className={cn("bg-muted inline-flex items-center gap-0.5 rounded-md p-0.5", className)}
      {...props}
    >
      {options.map((option) => {
        const active = value === option.value
        return (
          <Tooltip key={option.value} content={option.label}>
            <button
              type="button"
              role="radio"
              aria-checked={active}
              aria-label={option.label}
              onClick={() => onChange(option.value)}
              className={cn(
                "focus-visible:ring-ring inline-flex size-7 items-center justify-center rounded-sm transition-colors focus-visible:ring-2 focus-visible:outline-none",
                active ? "bg-background text-foreground shadow-sm" : "text-muted-foreground hover:text-foreground",
              )}
            >
              <option.Icon className="size-4" aria-hidden="true" />
            </button>
          </Tooltip>
        )
      })}
    </div>
  )
}
