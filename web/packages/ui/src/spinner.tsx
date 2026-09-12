import { LoaderCircle } from "lucide-react"

import { cn } from "./cn"

// The one spinner every surface shares. It is a status for a reader who cannot see it, so the
// label says what is being waited for.
export function Spinner({ label, className }: { label: string; className?: string }) {
  return <LoaderCircle role="status" aria-label={label} className={cn("size-4 shrink-0 animate-spin", className)} />
}
