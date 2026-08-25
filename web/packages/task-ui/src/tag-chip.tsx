import { TriangleAlert } from "lucide-react"

import type { Color, TagView } from "@open-planner/api-client"
import { cn, Tag, Tooltip } from "@open-planner/ui"

// Tailwind emits only the class names it can read literally in the source, so each colour is spelled
// out rather than composed from the name. Border and text share the rung, as they do on a branch tag.
const paletteColor: Record<Color, string> = {
  slate: "border-tag-slate text-tag-slate",
  red: "border-tag-red text-tag-red",
  orange: "border-tag-orange text-tag-orange",
  amber: "border-tag-amber text-tag-amber",
  yellow: "border-tag-yellow text-tag-yellow",
  green: "border-tag-green text-tag-green",
  teal: "border-tag-teal text-tag-teal",
  cyan: "border-tag-cyan text-tag-cyan",
  blue: "border-tag-blue text-tag-blue",
  indigo: "border-tag-indigo text-tag-indigo",
  violet: "border-tag-violet text-tag-violet",
  pink: "border-tag-pink text-tag-pink",
}

// Chips are the tallest thing on a task row, so they decide its height — and a list row owes its
// borders a whole number of pixels. 16 + 2 + 2 + 1 + 1.
const WHOLE_PIXELS = "py-0.5 leading-4"

const DANGLING = "border-muted-foreground/50 text-muted-foreground"

const danglingReason = (name: string) =>
  `${name} is not a tag on this branch — it was created elsewhere, renamed, or deleted. Editing this task's tags drops it.`

// `tag` is the registry entry the name resolves to, or `undefined` for a name this branch's registry
// does not hold.
export function TagChip({ name, tag }: { name: string; tag: TagView | undefined }) {
  if (tag === undefined) {
    return (
      <Tooltip content={danglingReason(name)}>
        <Tag className={cn(DANGLING, WHOLE_PIXELS)} dashed>
          <TriangleAlert aria-hidden className="size-3 shrink-0" />
          <span>{name}</span>
        </Tag>
      </Tooltip>
    )
  }
  const chip = <Tag className={cn(paletteColor[tag.color], WHOLE_PIXELS)}>{tag.display}</Tag>
  return tag.description === undefined ? chip : <Tooltip content={tag.description}>{chip}</Tooltip>
}
