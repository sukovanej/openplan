import { Badge } from "@/components/ui/badge"
import type { BranchState, ChangeKind } from "@/lib/api"
import { cn } from "@/lib/utils"

const kindClass: Record<ChangeKind, string> = {
  base: "",
  added: "border-emerald-500 text-emerald-600",
  modified: "border-sky-500 text-sky-600",
  deleted: "border-rose-500 text-rose-600 line-through",
}

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
            kindClass[branch.kind],
            branch.dirty && "border-amber-500 text-amber-600 no-underline",
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
  if (branch.kind !== "base") notes.push(branch.kind)
  if (branch.dirty) notes.push("uncommitted")
  return `${branch.branch}: ${notes.join(", ")}`
}
