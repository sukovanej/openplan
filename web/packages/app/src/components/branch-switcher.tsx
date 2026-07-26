import type { BranchState } from "@open-planner/api-client"

import { BranchTag } from "@/components/branch-tag"

// A task can live on several branches with diverging content; the switcher lets the reader pin any
// one. `selected` is the pinned branch, or `undefined` for the default read — which resolves to
// `headline`, the most recently changed branch, marked with a leading dot. Clicking the headline
// returns to that clean default (no `?branch=`); any other branch pins it.
export function BranchSwitcher({
  branches,
  selected,
  headline,
  onSelect,
}: {
  branches: ReadonlyArray<BranchState>
  selected: string | undefined
  headline: string
  onSelect: (branch: string | undefined) => void
}) {
  if (branches.length < 2) return null
  const active = selected ?? headline
  return (
    <div className="mb-4 flex flex-wrap items-center gap-1.5">
      {branches.map((branch) => {
        const isHeadline = branch.branch === headline
        return (
          <BranchTag
            key={branch.branch}
            branch={branch}
            headline={isHeadline}
            selected={active === branch.branch}
            onSelect={() => onSelect(isHeadline ? undefined : branch.branch)}
          />
        )
      })}
    </div>
  )
}
