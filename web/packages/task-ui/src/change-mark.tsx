import type { DocumentChangeKind } from "@openplan/api-client"
import { cn } from "@openplan/ui"

const kindText: Record<DocumentChangeKind, string> = {
  added: "text-change-added",
  modified: "text-change-modified",
  removed: "text-change-deleted",
}

export function ChangeMark({ kind, className }: { kind: DocumentChangeKind; className?: string }) {
  return <span className={cn("text-[11px] tracking-wide", kindText[kind], className)}>{kind}</span>
}
