import { BranchTag } from "@/components/branch-tag"
import type { BranchState, ChangeKind } from "@/lib/api"

const kindOrder: Record<ChangeKind, number> = { deleted: 0, added: 1, modified: 2, base: 3 }

export function BranchBadges({ branches, headline }: { branches: ReadonlyArray<BranchState>; headline: string }) {
  if (branches.length === 0) return null
  const ordered = [...branches].sort((a, b) => kindOrder[a.kind] - kindOrder[b.kind])
  return (
    <div className="flex items-center justify-end gap-1">
      {ordered.map((branch) => (
        <BranchTag key={branch.branch} branch={branch} headline={branches.length > 1 && branch.branch === headline} />
      ))}
    </div>
  )
}
