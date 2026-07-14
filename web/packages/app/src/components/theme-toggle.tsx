import { type LucideIcon, Monitor, Moon, Sun } from "lucide-react"
import type * as React from "react"

import { type ThemePreference, useTheme } from "@/lib/theme"
import { cn } from "@/lib/utils"

const OPTIONS: ReadonlyArray<{ value: ThemePreference; label: string; Icon: LucideIcon }> = [
  { value: "light", label: "Light", Icon: Sun },
  { value: "dark", label: "Dark", Icon: Moon },
  { value: "system", label: "System", Icon: Monitor },
]

export function ThemeToggle({ className, ...props }: React.ComponentProps<"div">) {
  const { preference, setPreference } = useTheme()
  return (
    <div
      role="radiogroup"
      aria-label="Theme"
      className={cn("bg-muted inline-flex items-center gap-0.5 rounded-md p-0.5", className)}
      {...props}
    >
      {OPTIONS.map(({ value, label, Icon }) => {
        const active = preference === value
        return (
          <button
            key={value}
            type="button"
            role="radio"
            aria-checked={active}
            aria-label={label}
            title={label}
            onClick={() => setPreference(value)}
            className={cn(
              "focus-visible:ring-ring inline-flex size-7 items-center justify-center rounded-sm transition-colors focus-visible:ring-2 focus-visible:outline-none",
              active
                ? "bg-background text-foreground shadow-sm"
                : "text-muted-foreground hover:text-foreground",
            )}
          >
            <Icon className="size-4" aria-hidden="true" />
          </button>
        )
      })}
    </div>
  )
}
