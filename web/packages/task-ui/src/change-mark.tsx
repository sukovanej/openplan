import type { ChangeKind } from "@openplan/api-client"
import { cn } from "@openplan/ui"

const kindText: Record<ChangeKind, string> = {
  base: "text-muted-foreground",
  added: "text-change-added",
  modified: "text-change-modified",
  deleted: "text-change-deleted",
}

export function ChangeMark({ kind, className }: { kind: ChangeKind; className?: string }) {
  return <span className={cn("text-[11px] tracking-wide", kindText[kind], className)}>{kind}</span>
}
