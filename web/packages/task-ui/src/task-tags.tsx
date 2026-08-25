import type { Metadata, TagView } from "@open-planner/api-client"
import { cn } from "@open-planner/ui"

import { tagsOf } from "./metadata"
import { TagChip } from "./tag-chip"

// `tags` is the branch's registry by name, or `undefined` while it is still being read — until it
// arrives, a name the branch does hold looks exactly like one it does not, so nothing is shown
// rather than every chip claiming to be dangling.
export function TaskTags({
  metadata,
  tags,
  className,
}: {
  metadata: Metadata
  tags: ReadonlyMap<string, TagView> | undefined
  className?: string
}) {
  const names = tagsOf(metadata)
  if (tags === undefined || names.length === 0) return null
  return (
    <div className={cn("flex flex-wrap items-center gap-1", className)}>
      {names.map((name) => (
        <TagChip key={name} name={name} tag={tags.get(name)} />
      ))}
    </div>
  )
}
