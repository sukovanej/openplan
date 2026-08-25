import { TriangleAlert, X } from "lucide-react"

import type { Color, TagView } from "@open-planner/api-client"
import { cn, Tag, Tooltip } from "@open-planner/ui"

// A chip fills with its colour, so its ink is whatever reads on that fill rather than a theme
// colour. Six of the palette are bright enough in light mode to need dark ink; in dark mode every
// rung is a light one, where only dark ink reads. Tailwind emits only the class names it can read
// literally in the source, so each colour spells out its own pair.
const paletteFill: Record<Color, string> = {
  slate: "bg-tag-slate text-white dark:text-black",
  red: "bg-tag-red text-white dark:text-black",
  orange: "bg-tag-orange text-black",
  amber: "bg-tag-amber text-black",
  yellow: "bg-tag-yellow text-black",
  green: "bg-tag-green text-black",
  teal: "bg-tag-teal text-black",
  cyan: "bg-tag-cyan text-black",
  blue: "bg-tag-blue text-white dark:text-black",
  indigo: "bg-tag-indigo text-white dark:text-black",
  violet: "bg-tag-violet text-white dark:text-black",
  pink: "bg-tag-pink text-white dark:text-black",
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
    <Tag
      className={cn(tag === undefined ? DANGLING : `${paletteFill[tag.color]} border-transparent`, WHOLE_PIXELS)}
      dashed={tag === undefined}
    >
      {tag === undefined && <TriangleAlert aria-hidden className="size-3 shrink-0" />}
      <span>{label}</span>
      {onRemove !== undefined && (
        <button
          type="button"
          aria-label={`Remove ${label}`}
          onClick={onRemove}
          className="-mr-0.5 shrink-0 opacity-60 transition-opacity hover:opacity-100 focus-visible:opacity-100 focus-visible:outline-none"
        >
          <X className="size-3" />
        </button>
      )}
    </Tag>
  )
  const explains = tag === undefined ? danglingReason(name, branch) : tag.description
  return explains === undefined ? chip : <Tooltip content={explains}>{chip}</Tooltip>
}
