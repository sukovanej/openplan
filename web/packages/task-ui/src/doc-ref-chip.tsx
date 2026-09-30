import { FileText } from "lucide-react"
import { Link } from "react-router-dom"

import type { DocRef } from "@openplan/api-client"
import { cn } from "@openplan/ui"

const CHIP =
  "not-prose relative -top-px mx-0.5 inline-flex max-w-full items-center gap-1.5 rounded-md border px-1.5 py-0.5 align-middle text-sm font-medium leading-5 no-underline transition-colors"

// A chip names its doc by the name alone. A reference the store cannot resolve renders dashed.
export function DocRefChip({ to, name, doc }: { to: string; name: string; doc: DocRef | undefined }) {
  return (
    <Link
      to={to}
      className={cn(
        CHIP,
        doc === undefined
          ? "border-border border-dashed text-muted-foreground hover:bg-muted/40"
          : "border-border bg-muted/40 text-foreground hover:bg-muted",
      )}
    >
      <FileText className="size-3.5 shrink-0 opacity-70" />
      <span className="truncate">{name}</span>
    </Link>
  )
}
