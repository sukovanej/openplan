import type { BranchState, ChangeKind } from "@/lib/api"
import { cn } from "@/lib/utils"

// Border and text share one hue per change kind, so a tag never pairs a grey border with coloured text.
const kindColor: Record<ChangeKind, string> = {
  base: "border-foreground text-foreground",
  added: "border-emerald-600 text-emerald-600",
  modified: "border-sky-600 text-sky-600",
  deleted: "border-rose-600 text-rose-600 line-through",
}

// The single branch tag used everywhere a branch is shown. Read-only (list badges) when `onSelect`
// is omitted, a toggle button (detail switcher) when it is given.
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
  const interactive = onSelect !== undefined
  const className = cn(
    "inline-flex items-center gap-1 rounded-md border px-2 py-1 font-mono text-[11px] leading-tight",
    "whitespace-nowrap transition-opacity",
    kindColor[branch.kind],
    branch.dirty && "border-dashed",
    interactive && "cursor-pointer",
    interactive && !selected && "opacity-55 hover:opacity-100",
    interactive && selected && "font-semibold",
  )
  const title = branchTitle(branch, headline)
  const content = (
    <>
      {headline && <Dot className="bg-current" />}
      <span>{branch.branch}</span>
    </>
  )
  return onSelect === undefined ? (
    <span title={title} className={className}>
      {content}
    </span>
  ) : (
    <button type="button" onClick={onSelect} aria-pressed={selected} title={title} className={className}>
      {content}
    </button>
  )
}

function Dot({ className }: { className: string }) {
  return <span aria-hidden className={cn("size-1.5 shrink-0 rounded-full", className)} />
}

function branchTitle(branch: BranchState, headline: boolean): string {
  const notes: Array<string> = [branch.status]
  if (headline) notes.push("latest")
  if (branch.kind !== "base") notes.push(branch.kind)
  if (branch.dirty) notes.push("uncommitted")
  return `${branch.branch}: ${notes.join(", ")}`
}
