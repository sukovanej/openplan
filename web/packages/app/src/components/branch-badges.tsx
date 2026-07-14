import { Badge } from "@/components/ui/badge"
import type { BranchState } from "@/lib/api"
import { cn } from "@/lib/utils"

export function BranchBadges({ branches }: { branches: ReadonlyArray<BranchState> }) {
  if (branches.length === 0) return null
  return (
    <div className="flex flex-wrap items-center justify-end gap-1">
      {branches.map((branch) => (
        <Badge
          key={branch.branch}
          variant="outline"
          className={cn(
            "font-mono text-[10px] leading-tight",
            branch.conflicted && "border-destructive text-destructive",
            !branch.conflicted && branch.dirty && "border-amber-500 text-amber-600",
          )}
          title={branchTitle(branch)}
        >
          {branch.branch}
          {branch.dirty ? " ●" : ""}
        </Badge>
      ))}
    </div>
  )
}

function branchTitle(branch: BranchState): string {
  const notes: Array<string> = [branch.status]
  if (branch.dirty) notes.push("uncommitted")
  if (branch.conflicted) notes.push("conflict")
  return `${branch.branch}: ${notes.join(", ")}`
}
