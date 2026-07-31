import type { BranchState, ChangeKind } from "@open-planner/api-client"
import { cn, Tag, Tooltip } from "@open-planner/ui"

import { fieldValue } from "./metadata"

// Border and text share one hue per change kind, so a tag never pairs a grey border with coloured text.
const kindColor: Record<ChangeKind, string> = {
  base: "border-foreground text-foreground",
  added: "border-change-added text-change-added",
  modified: "border-change-modified text-change-modified",
  deleted: "border-change-deleted text-change-deleted line-through",
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
    <Tooltip content={branchTitle(branch, headline)}>
      <Tag className={kindColor[branch.kind]} dashed={branch.dirty} selected={selected} onSelect={onSelect}>
        {headline && <Dot className="bg-current" />}
        <span>{branch.branch}</span>
      </Tag>
    </Tooltip>
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
