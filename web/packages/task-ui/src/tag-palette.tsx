import type { Color } from "@open-planner/api-client"
import { cn } from "@open-planner/ui"

// Tailwind emits only the class names it can read literally in the source, so each colour spells out
// a swatch of its own rather than composing one from the name.
const swatch: Record<Color, string> = {
  slate: "bg-tag-slate",
  red: "bg-tag-red",
  orange: "bg-tag-orange",
  amber: "bg-tag-amber",
  yellow: "bg-tag-yellow",
  green: "bg-tag-green",
  teal: "bg-tag-teal",
  cyan: "bg-tag-cyan",
  blue: "bg-tag-blue",
  indigo: "bg-tag-indigo",
  violet: "bg-tag-violet",
  pink: "bg-tag-pink",
}

// The palette in the order the picker lays it out. The map above is exhaustive by its type, so a
// colour the API can carry cannot miss the picker.
export const TAG_COLORS = Object.keys(swatch) as ReadonlyArray<Color>

export function ColorDot({ color, className }: { color: Color; className?: string }) {
  return <span aria-hidden className={cn("size-2.5 shrink-0 rounded-full", swatch[color], className)} />
}

export function ColorPicker({ value, onPick }: { value: Color; onPick: (color: Color) => void }) {
  return (
    <div role="radiogroup" aria-label="Tag colour" className="flex flex-wrap items-center gap-1.5">
      {TAG_COLORS.map((color) => (
        <button
          key={color}
          type="button"
          role="radio"
          aria-checked={color === value}
          aria-label={color}
          onClick={() => onPick(color)}
          className={cn(
            "focus-visible:ring-ring size-5 rounded-full transition-transform focus-visible:ring-2 focus-visible:outline-none",
            swatch[color],
            color === value ? "ring-foreground/50 ring-offset-background ring-2 ring-offset-2" : "hover:scale-110",
          )}
        />
      ))}
    </div>
  )
}
