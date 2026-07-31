import { Monitor, Moon, Sun } from "lucide-react"
import type * as React from "react"

import { type IconToggleOption, IconToggleGroup } from "@open-planner/ui"

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
