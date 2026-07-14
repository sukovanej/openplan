import type { ReactNode } from "react"

import type { BranchState, ChangeKind } from "@/lib/api"
import { cn } from "@/lib/utils"

const kindClass: Record<ChangeKind, string> = {
  base: "",
  added: "border-emerald-500 text-emerald-600",
  modified: "border-sky-500 text-sky-600",
  deleted: "border-rose-500 text-rose-600 line-through",
}

// A task can live on several branches with diverging content; the switcher lets the reader pin any
// one. `undefined` selects the headline (current-worktree) version the server picks by default.
export function BranchSwitcher(
  { branches, selected, onSelect }: {
    branches: ReadonlyArray<BranchState>
    selected: string | undefined
    onSelect: (branch: string | undefined) => void
  },
) {
  if (branches.length < 2) return null
  return (
    <div className="mb-4 flex flex-wrap items-center gap-1.5">
      <span className="text-muted-foreground text-xs">branch</span>
      <Chip active={selected === undefined} onClick={() => onSelect(undefined)}>current</Chip>
      {branches.map((branch) => (
        <Chip
          key={branch.branch}
          active={selected === branch.branch}
          onClick={() => onSelect(branch.branch)}
          className={cn(kindClass[branch.kind], branch.dirty && "border-amber-500 text-amber-600")}
          title={branchTitle(branch)}
        >
          {branch.branch}
          {branch.dirty ? " ●" : ""}
        </Chip>
      ))}
    </div>
  )
}

function Chip(
  { active, onClick, className, title, children }: {
    active: boolean
    onClick: () => void
    className?: string
    title?: string
    children: ReactNode
  },
) {
  return (
    <button
      type="button"
      onClick={onClick}
      title={title}
      aria-pressed={active}
      className={cn(
        "rounded-md border px-2 py-0.5 font-mono text-[11px] leading-tight transition-colors",
        active ? "border-foreground bg-foreground text-background" : "hover:bg-muted",
        !active && className,
      )}
    >
      {children}
    </button>
  )
}

function branchTitle(branch: BranchState): string {
  const notes: Array<string> = [branch.status]
  if (branch.kind !== "base") notes.push(branch.kind)
  if (branch.dirty) notes.push("uncommitted")
  return `${branch.branch}: ${notes.join(", ")}`
}
