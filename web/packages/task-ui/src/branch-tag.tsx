import type { BranchState, ChangeKind } from "@openplan/api-client"
import { Tag, Tooltip } from "@openplan/ui"

import { fieldValue } from "./metadata"

// Border and text share one hue per change kind, so a tag never pairs a grey border with coloured text.
const kindColor: Record<ChangeKind, string> = {
  base: "border-foreground text-foreground",
  added: "border-change-added text-change-added",
  modified: "border-change-modified text-change-modified",
  deleted: "border-change-deleted text-change-deleted line-through",
}

// A person never checks the rolling-updates branch out and never types its name, so the tag says
// what the branch is instead of what it is called.
export const ROLLING_UPDATES_LABEL = "Rolling updates"

// The single branch tag used everywhere a branch is shown.
export function BranchTag({
  branch,
  headline = false,
  selected = false,
  rollingUpdates,
  onSelect,
}: {
  branch: BranchState
  headline?: boolean
  selected?: boolean
  rollingUpdates?: string | null
  onSelect?: () => void
}) {
  const rolling = branch.branch === rollingUpdates
  return (
    <Tooltip content={branchTitle(branch, headline, rolling)}>
      <Tag className={kindColor[branch.kind]} dashed={branch.dirty} selected={selected} onSelect={onSelect}>
        {headline && <span aria-hidden className="size-1.5 shrink-0 rounded-full bg-current" />}
        <span>{rolling ? ROLLING_UPDATES_LABEL : branch.branch}</span>
      </Tag>
    </Tooltip>
  )
}

const branchStatusText = (status: BranchState["status"]): string => fieldValue(status) ?? "unreadable"

function branchTitle(branch: BranchState, headline: boolean, rolling: boolean): string {
  const notes: Array<string> = [branchStatusText(branch.status)]
  if (headline) notes.push("latest")
  if (branch.kind !== "base") notes.push(branch.kind)
  if (branch.dirty) notes.push("uncommitted")
  if (rolling) notes.push("unpublished")
  return `${branch.branch}: ${notes.join(", ")}`
}
