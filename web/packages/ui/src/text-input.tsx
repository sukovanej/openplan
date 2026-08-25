import type * as React from "react"

import { cn } from "./cn"

export function TextInput({ className, ...props }: React.ComponentProps<"input">) {
  return (
    <input
      type="text"
      autoComplete="off"
      spellCheck={false}
      className={cn(
        "border-input focus:border-foreground/20 bg-background placeholder:text-muted-foreground h-9 min-w-0 rounded-md border px-2.5 text-sm transition-colors outline-none",
        className,
      )}
      {...props}
    />
  )
}
