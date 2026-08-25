import type { ReactNode } from "react"

import type { Metadata, TagView } from "@open-planner/api-client"
import { cn } from "@open-planner/ui"

import { tagsOf } from "./metadata"
import { TagChip } from "./tag-chip"

// `tags` is the branch's registry by name, or `undefined` while it is still being read — until it
// arrives, a name the branch does hold looks exactly like one it does not, so nothing is shown
// rather than every chip claiming to be dangling. `onRemove` makes each chip editable; `trailing`
// takes whatever control the caller puts after the chips, and keeps the row on screen for a task
// that carries none.
export function TaskTags({
  metadata,
  tags,
  onRemove,
  trailing,
  className,
}: {
  metadata: Metadata
  tags: ReadonlyMap<string, TagView> | undefined
  onRemove?: (name: string) => void
  trailing?: ReactNode
  className?: string
}) {
  const names = tagsOf(metadata)
  const chips = tags === undefined ? [] : names
  if (chips.length === 0 && trailing === undefined) return null
  return (
    <div className={cn("flex flex-wrap items-center gap-1", className)}>
      {chips.map((name) => (
        <TagChip
          key={name}
          name={name}
          tag={tags?.get(name)}
          onRemove={onRemove === undefined ? undefined : () => onRemove(name)}
        />
      ))}
      {trailing}
    </div>
  )
}
