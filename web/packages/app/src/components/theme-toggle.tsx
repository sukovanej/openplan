import { Monitor, Moon, Sun } from "lucide-react"
import type * as React from "react"

import { Button, cn, type IconToggleOption, IconToggleGroup, Tooltip } from "@openplan/ui"

import { type ThemePreference, useTheme } from "../lib/theme"

const OPTIONS: ReadonlyArray<IconToggleOption<ThemePreference>> = [
  { value: "light", label: "Light", Icon: Sun },
  { value: "dark", label: "Dark", Icon: Moon },
  { value: "system", label: "System", Icon: Monitor },
]

export function ThemeToggle(props: Omit<React.ComponentProps<"div">, "onChange">) {
  const { preference, setPreference } = useTheme()
  return <IconToggleGroup label="Theme" options={OPTIONS} value={preference} onChange={setPreference} {...props} />
}

// A phone has no room for the three side by side, so one button steps through them.
export function ThemeButton({ className }: { className?: string }) {
  const { preference, setPreference } = useTheme()
  const at = OPTIONS.findIndex((option) => option.value === preference)
  const { label, Icon } = OPTIONS[at]
  const next = OPTIONS[(at + 1) % OPTIONS.length]
  return (
    <Tooltip content={`Theme: ${label}`}>
      <Button
        size="icon"
        aria-label={`Theme: ${label}. Switch to ${next.label}.`}
        onClick={() => setPreference(next.value)}
        className={cn("text-muted-foreground size-9", className)}
      >
        <Icon className="size-5" aria-hidden />
      </Button>
    </Tooltip>
  )
}
