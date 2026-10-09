import { ChevronsUpDown } from "lucide-react"
import type { ComponentProps } from "react"

import { cn } from "./cn"
import { CONTROL_HEIGHT } from "./control"

export function MenuTrigger({ open, className, children, ...props }: ComponentProps<"button"> & { open: boolean }) {
  return (
    <button
      type="button"
      aria-haspopup="listbox"
      aria-expanded={open}
      className={cn(
        "hover:bg-muted focus-visible:ring-ring inline-flex min-w-0 items-center gap-1.5 rounded-md border px-2 text-sm transition-colors focus-visible:ring-2 focus-visible:outline-none",
        CONTROL_HEIGHT,
        className,
      )}
      {...props}
    >
      {children}
      <ChevronsUpDown className="text-muted-foreground size-3.5 shrink-0" aria-hidden />
    </button>
  )
}
