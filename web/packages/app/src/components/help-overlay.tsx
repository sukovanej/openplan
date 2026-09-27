import { type RefObject, useLayoutEffect, useRef, useState } from "react"

import { cn, Dialog, Kbd } from "@openplan/ui"

import { bindings, type HelpEntry, helpGroups } from "../lib/keys"

const GROUPS = helpGroups(bindings)

const COLUMN_WIDTH_PX = 288
const COLUMN_GAP_PX = 32
// The overlay keeps this much of the window free around the dialog.
const WINDOW_MARGIN_PX = 32

interface Layout {
  readonly columns: number
  readonly maxHeight: number | undefined
  // Each group fits in a column, so no group needs to break across two.
  readonly whole: boolean
}

export function HelpOverlay({ open, onClose }: { open: boolean; onClose: () => void }) {
  const scroller = useRef<HTMLDivElement>(null)
  const body = useRef<HTMLDivElement>(null)
  const { columns, maxHeight, whole } = useLayout(scroller, body, open)
  return (
    <Dialog open={open} onClose={onClose} title="Keyboard shortcuts" className="w-auto max-w-none">
      <div ref={scroller} className="overflow-y-auto" style={{ maxHeight }}>
        <div
          ref={body}
          style={{
            columnCount: columns,
            columnGap: COLUMN_GAP_PX,
            width: columns * COLUMN_WIDTH_PX + (columns - 1) * COLUMN_GAP_PX,
          }}
        >
          {GROUPS.map((group) => (
            <section key={group.name} className={cn("pb-5 last:pb-0", whole && "break-inside-avoid")}>
              <h3 className="text-muted-foreground mb-2 break-after-avoid text-xs font-medium tracking-wide uppercase">
                {group.name}
              </h3>
              <ul className="space-y-1.5">
                {group.entries.map((entry) => (
                  <HelpRow key={entry.id} entry={entry} />
                ))}
              </ul>
            </section>
          ))}
        </div>
      </div>
    </Dialog>
  )
}

// A column takes whole groups until the next one would pass the bottom of the window, and the
// columns stop where the window's width does. A group taller than the window breaks between its
// rows, and what still does not fit scrolls.
function useLayout(
  scroller: RefObject<HTMLDivElement | null>,
  body: RefObject<HTMLDivElement | null>,
  open: boolean,
): Layout {
  const [layout, setLayout] = useState<Layout>({ columns: 1, maxHeight: undefined, whole: true })
  useLayoutEffect(() => {
    const outer = scroller.current
    const inner = body.current
    const dialog = outer?.closest<HTMLElement>("[role=dialog]")
    if (!open || outer === null || inner === null || dialog === null || dialog === undefined) return
    const fit = () => {
      const height = window.innerHeight - WINDOW_MARGIN_PX - (dialog.offsetHeight - outer.offsetHeight)
      const width = window.innerWidth - WINDOW_MARGIN_PX - (dialog.offsetWidth - outer.offsetWidth)
      const widest = Math.max(1, Math.floor((width + COLUMN_GAP_PX) / (COLUMN_WIDTH_PX + COLUMN_GAP_PX)))
      const heights = [...inner.children].map(groupHeight)
      const whole = heights.every((group) => group <= height)
      const needed = whole
        ? columnsFor(heights, height)
        : Math.ceil(heights.reduce((sum, group) => sum + group, 0) / height)
      const columns = Math.min(needed, widest)
      setLayout((held) =>
        held.columns === columns && held.maxHeight === height && held.whole === whole
          ? held
          : { columns, maxHeight: height, whole },
      )
    }
    fit()
    window.addEventListener("resize", fit)
    return () => window.removeEventListener("resize", fit)
  }, [scroller, body, open])
  return layout
}

const outerHeight = (element: Element): number => {
  const style = getComputedStyle(element)
  return (element as HTMLElement).offsetHeight + parseFloat(style.marginTop) + parseFloat(style.marginBottom)
}

// A group split across columns reports the height of one of its parts, but a heading and a row never
// split, so the group is the sum of them.
function groupHeight(group: Element): number {
  const [heading, rows] = group.children
  const padding = parseFloat(getComputedStyle(group).paddingBottom)
  return outerHeight(heading) + [...rows.children].reduce((sum, row) => sum + outerHeight(row), 0) + padding
}

function columnsFor(heights: ReadonlyArray<number>, height: number): number {
  let columns = 1
  let filled = 0
  for (const group of heights) {
    if (filled > 0 && filled + group > height) {
      columns++
      filled = 0
    }
    filled += group
  }
  return columns
}

function HelpRow({ entry }: { entry: HelpEntry }) {
  return (
    <li className="flex break-inside-avoid items-center justify-between gap-4">
      <span className="text-foreground/90 text-sm">{entry.label}</span>
      <span className="flex items-center gap-1">
        {entry.keys.map((token, index) => (
          <Kbd key={index} token={token} />
        ))}
      </span>
    </li>
  )
}
