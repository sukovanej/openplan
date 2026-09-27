import type { LucideIcon } from "lucide-react"
import type * as React from "react"

import { cn } from "./cn"
import { segment, SEGMENT_GROUP } from "./control"
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
    <div role="radiogroup" aria-label={label} className={cn(SEGMENT_GROUP, className)} {...props}>
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
              className={cn(segment(active), "w-7")}
            >
              <option.Icon className="size-4" aria-hidden="true" />
            </button>
          </Tooltip>
        )
      })}
    </div>
  )
}
