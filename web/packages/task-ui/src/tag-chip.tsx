import { TriangleAlert, X } from "lucide-react"

import type { Color, TagView } from "@open-planner/api-client"
import { cn, Tag, Tooltip } from "@open-planner/ui"

// A chip wears its colour as a wash behind the name rather than as a fill: at full strength the
// twelve read as blocks of shouting on a light page. The ink stays the palette rung, which was tuned
// to read on the page background — a 15% wash barely moves that, so both themes keep one recipe.
// Tailwind emits only the class names it can read literally in the source, so each colour spells out
// its own trio.
const paletteChip: Record<Color, string> = {
  slate: "bg-tag-slate/15 text-tag-slate border-tag-slate/30",
  red: "bg-tag-red/15 text-tag-red border-tag-red/30",
  orange: "bg-tag-orange/15 text-tag-orange border-tag-orange/30",
  amber: "bg-tag-amber/15 text-tag-amber border-tag-amber/30",
  yellow: "bg-tag-yellow/15 text-tag-yellow border-tag-yellow/30",
  green: "bg-tag-green/15 text-tag-green border-tag-green/30",
  teal: "bg-tag-teal/15 text-tag-teal border-tag-teal/30",
  cyan: "bg-tag-cyan/15 text-tag-cyan border-tag-cyan/30",
  blue: "bg-tag-blue/15 text-tag-blue border-tag-blue/30",
  indigo: "bg-tag-indigo/15 text-tag-indigo border-tag-indigo/30",
  violet: "bg-tag-violet/15 text-tag-violet border-tag-violet/30",
  pink: "bg-tag-pink/15 text-tag-pink border-tag-pink/30",
}

// Chips are the tallest thing on a task row, so they decide its height — and a list row owes its
// borders a whole number of pixels. 16 + 2 + 2 + 1 + 1.
const WHOLE_PIXELS = "py-0.5 leading-4"

const DANGLING = "border-muted-foreground/50 text-muted-foreground"

// A task can carry a name that one branch registers and another does not, so the reader is owed the
// branch this was resolved against. "not a tag on this branch" reads as a claim about the version on
// screen, which is a different branch as often as not.
const danglingReason = (name: string, branch: string | undefined) =>
  branch === undefined
    ? `${name} is not a tag in this project's registry. It may be registered on another branch, or it was renamed or deleted. Editing this task's tags drops the name.`
    : `${name} is not a tag on ${branch}, the branch an edit to this task writes to. It may be registered on another branch, or it was renamed or deleted. Editing this task's tags drops the name.`

// `tag` is the registry entry the name resolves to, or `undefined` for a name that registry does not
// hold; `branch` names the branch it was read from, or `undefined` for the served worktree's.
// `onRemove` turns the chip into an editable one: taking the tag off the task is the only thing a
// chip does, and it is the only thing a dangling one can do.
export function TagChip({
  name,
  tag,
  branch,
  onRemove,
}: {
  name: string
  tag: TagView | undefined
  branch?: string
  onRemove?: () => void
}) {
  const label = tag?.display ?? name
  const chip = (
    <Tag className={cn(tag === undefined ? DANGLING : paletteChip[tag.color], WHOLE_PIXELS)} dashed={tag === undefined}>
      {tag === undefined && <TriangleAlert aria-hidden className="size-3 shrink-0" />}
      <span>{label}</span>
      {onRemove !== undefined && (
        <button
          type="button"
          aria-label={`Remove ${label}`}
          onClick={onRemove}
          className="-mr-0.5 shrink-0 cursor-pointer opacity-60 transition-opacity hover:opacity-100 focus-visible:opacity-100 focus-visible:outline-none"
        >
          <X className="size-3" />
        </button>
      )}
    </Tag>
  )
  const explains = tag === undefined ? danglingReason(name, branch) : tag.description
  return explains === undefined ? chip : <Tooltip content={explains}>{chip}</Tooltip>
}
