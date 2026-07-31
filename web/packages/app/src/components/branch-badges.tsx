import type { BranchState, ChangeKind } from "@open-planner/api-client"

import { cn } from "../lib/utils"
import { BranchTag } from "./branch-tag"

const kindOrder: Record<ChangeKind, number> = { deleted: 0, added: 1, modified: 2, base: 3 }

export function BranchBadges({
  branches,
  headline,
  className,
}: {
  branches: ReadonlyArray<BranchState>
  headline: string
  className?: string
}) {
  if (branches.length === 0) return null
  const ordered = [...branches].sort((a, b) => kindOrder[a.kind] - kindOrder[b.kind])
  return (
    <div className={cn("flex flex-wrap items-center gap-1", className)}>
      {ordered.map((branch) => (
        <BranchTag key={branch.branch} branch={branch} headline={branches.length > 1 && branch.branch === headline} />
      ))}
    </div>
  )
}
