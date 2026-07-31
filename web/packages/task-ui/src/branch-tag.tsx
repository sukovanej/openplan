import type { BranchState, ChangeKind } from "@open-planner/api-client"
import { cn, Tag } from "@open-planner/ui"

import { fieldValue } from "./metadata"

// Border and text share one hue per change kind, so a tag never pairs a grey border with coloured text.
const kindColor: Record<ChangeKind, string> = {
  base: "border-foreground text-foreground",
  added: "border-emerald-600 text-emerald-600",
  modified: "border-sky-600 text-sky-600",
  deleted: "border-rose-600 text-rose-600 line-through",
}

// The single branch tag used everywhere a branch is shown.
export function BranchTag({
  branch,
  headline = false,
  selected = false,
  onSelect,
}: {
  branch: BranchState
  headline?: boolean
  selected?: boolean
  onSelect?: () => void
}) {
  return (
    <Tag
      title={branchTitle(branch, headline)}
      className={kindColor[branch.kind]}
      dashed={branch.dirty}
      selected={selected}
      onSelect={onSelect}
    >
      {headline && <Dot className="bg-current" />}
      <span>{branch.branch}</span>
    </Tag>
  )
}

function Dot({ className }: { className: string }) {
  return <span aria-hidden className={cn("size-1.5 shrink-0 rounded-full", className)} />
}

const statusText = (status: BranchState["status"]): string => fieldValue(status) ?? "unreadable"

function branchTitle(branch: BranchState, headline: boolean): string {
  const notes: Array<string> = [statusText(branch.status)]
  if (headline) notes.push("latest")
  if (branch.kind !== "base") notes.push(branch.kind)
  if (branch.dirty) notes.push("uncommitted")
  return `${branch.branch}: ${notes.join(", ")}`
}
