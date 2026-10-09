import type { ReactNode } from "react"

import type { Metadata, TagView } from "@openplan/api-client"
import { cn } from "@openplan/ui"

import { tagsOf } from "./metadata"
import { TagChip } from "./tag-chip"

interface TagListProps {
  tags: ReadonlyMap<string, TagView> | undefined
  onRemove?: (name: string) => void
  trailing?: ReactNode
  className?: string
}

export function TaskTags({ metadata, ...list }: TagListProps & { metadata: Metadata }) {
  return <TagList names={tagsOf(metadata)} {...list} />
}

// `tags` is the project's registry by name, or `undefined` while it is still being read — until it
// arrives, a name the registry does hold looks exactly like one it does not, so nothing is shown
// rather than every chip claiming to be dangling. `onRemove` makes each chip editable; `trailing`
// takes whatever control the caller puts after the chips, and keeps the row on screen for a list
// that holds none.
export function TagList({
  names,
  tags,
  onRemove,
  trailing,
  className,
}: TagListProps & { names: ReadonlyArray<string> }) {
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
