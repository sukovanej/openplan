import type { ComponentProps, ElementType } from "react"

import { cn } from "./cn"

// The current row loses its own bottom border — the outline draws that edge — and gets the outline as
// an absolutely-positioned overlay, so neither participates in flow and the row keeps its height
// whether or not it is current. The overlay is positioned against the row's padding box, which stops
// 1px short of each separator, so both offsets are -1px: the top edge lands on the separator above,
// the bottom edge on the row's own, exactly where the neighbouring rows draw theirs.
const DIVIDED_ACTIVE =
  "border-transparent bg-muted/30 after:pointer-events-none after:absolute after:inset-x-0 after:-top-px after:-bottom-px after:border after:border-accent-line/40 after:content-['']"

// The same treatment under the pointer, spelled out because Tailwind only emits classes it can read
// literally in the source.
const DIVIDED_HOVER =
  "hover:border-transparent hover:bg-muted/30 hover:after:pointer-events-none hover:after:absolute hover:after:inset-x-0 hover:after:-top-px hover:after:-bottom-px hover:after:border hover:after:border-accent-line/40 hover:after:content-['']"

// The same again, only inside an ancestor marked `data-pointer="free"`. A list that hands its rows
// between the keyboard and the pointer then changes one attribute, and renders none of its rows again.
const DIVIDED_FREE_HOVER =
  "in-data-[pointer=free]:hover:border-transparent in-data-[pointer=free]:hover:bg-muted/30 in-data-[pointer=free]:hover:after:pointer-events-none in-data-[pointer=free]:hover:after:absolute in-data-[pointer=free]:hover:after:inset-x-0 in-data-[pointer=free]:hover:after:-top-px in-data-[pointer=free]:hover:after:-bottom-px in-data-[pointer=free]:hover:after:border in-data-[pointer=free]:hover:after:border-accent-line/40 in-data-[pointer=free]:hover:after:content-['']"

const VARIANTS = {
  // Separated by a rule from the row below, and outlined in place when current. Callers own the
  // padding, and owe it a whole number of pixels: a fractional row height puts these borders on half
  // device pixels and the row shimmers on Retina as it is selected.
  divided: {
    base: "relative border-b transition-colors",
    active: DIVIDED_ACTIVE,
    hover: DIVIDED_HOVER,
    freeHover: DIVIDED_FREE_HOVER,
  },
  // A pickable row in a scrolling list, filled rather than outlined when current.
  option: {
    base: "flex items-center gap-2 rounded-md px-2 py-1.5 text-sm",
    active: "bg-accent",
    hover: "hover:bg-muted/50",
    freeHover: "in-data-[pointer=free]:hover:bg-muted/50",
  },
} as const

type RowProps<T extends ElementType> = {
  as?: T
  variant?: keyof typeof VARIANTS
  active?: boolean
  hoverable?: boolean | "while-free"
  last?: boolean
} & Omit<ComponentProps<T>, "as">

export function Row<T extends ElementType = "div">({
  as,
  variant = "divided",
  active = false,
  hoverable = false,
  last = false,
  className,
  ...props
}: RowProps<T>) {
  const Component = (as ?? "div") as ElementType
  const styles = VARIANTS[variant]
  return (
    <Component
      className={cn(
        styles.base,
        // The last row has no row below to separate it from.
        last && "border-transparent",
        active && styles.active,
        // Hover would otherwise paint over the current row's own treatment, which is the one the
        // pointer is about to hand it anyway.
        hoverable === true && !active && styles.hover,
        hoverable === "while-free" && !active && styles.freeHover,
        className,
      )}
      {...props}
    />
  )
}
