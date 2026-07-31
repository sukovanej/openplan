import { cva, type VariantProps } from "class-variance-authority"
import type * as React from "react"

import { cn } from "./cn"

const button = cva(
  "focus-visible:ring-ring inline-flex shrink-0 items-center rounded-md transition-colors focus-visible:ring-2 focus-visible:outline-none",
  {
    variants: {
      variant: {
        ghost: "text-muted-foreground/60 hover:bg-muted hover:text-foreground",
        accent: "text-accent-line hover:bg-accent-line/10 font-medium",
        danger: "hover:bg-danger-border/15",
      },
      size: {
        sm: "gap-1.5 px-2 py-1 text-xs",
        icon: "size-7 justify-center",
      },
    },
    defaultVariants: { variant: "ghost", size: "sm" },
  },
)

export function Button({
  variant,
  size,
  className,
  type = "button",
  ...props
}: React.ComponentProps<"button"> & VariantProps<typeof button>) {
  return <button type={type} className={cn(button({ variant, size }), className)} {...props} />
}
