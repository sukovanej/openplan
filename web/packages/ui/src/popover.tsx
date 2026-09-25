import { type KeyboardEvent, type ReactNode, type RefObject, useEffect, useLayoutEffect, useRef, useState } from "react"

import { cn } from "./cn"

const GAP = 6

// Fixed to the viewport, like the tooltip, so a row, a meta line or a scrolling column that clips
// its overflow cannot cut the panel off. It opens below its anchor, and above it when only that fits.
export function Popover({
  anchor,
  label,
  align = "start",
  onClose,
  className,
  children,
}: {
  anchor: RefObject<HTMLElement | null>
  label: string
  align?: "start" | "end"
  onClose: () => void
  className?: string
  children: ReactNode
}) {
  const panel = useRef<HTMLDivElement>(null)
  const [at, setAt] = useState<{ left: number; top: number }>()

  useLayoutEffect(() => {
    const place = () => {
      const from = anchor.current?.getBoundingClientRect()
      const box = panel.current?.getBoundingClientRect()
      if (from === undefined || box === undefined) return
      const wanted = align === "start" ? from.left : from.right - box.width
      const below = from.bottom + GAP
      const above = from.top - box.height - GAP
      const next = {
        left: Math.max(GAP, Math.min(wanted, window.innerWidth - box.width - GAP)),
        top: below + box.height > window.innerHeight - GAP && above >= GAP ? above : below,
      }
      setAt((current) => (current?.left === next.left && current.top === next.top ? current : next))
    }
    place()
    window.addEventListener("scroll", place, true)
    window.addEventListener("resize", place)
    return () => {
      window.removeEventListener("scroll", place, true)
      window.removeEventListener("resize", place)
    }
  }, [anchor, align, children])

  useEffect(() => panel.current?.focus(), [])

  useEffect(() => {
    const onDown = (event: MouseEvent) => {
      const target = event.target as Node
      if (panel.current?.contains(target) || anchor.current?.contains(target)) return
      onClose()
    }
    document.addEventListener("mousedown", onDown)
    return () => document.removeEventListener("mousedown", onDown)
  }, [anchor, onClose])

  // The app's key dispatcher skips a key that is already handled, so Esc closes the panel and not
  // the page behind it.
  const onKeyDown = (event: KeyboardEvent) => {
    if (event.key !== "Escape") return
    event.preventDefault()
    anchor.current?.focus()
    onClose()
  }

  return (
    <div
      ref={panel}
      role="dialog"
      aria-label={label}
      tabIndex={-1}
      data-keys-ignore
      onKeyDown={onKeyDown}
      style={at}
      // It opens inside headers that set their own case, weight and tracking, and states the whole
      // of its own type rather than inheriting any of that.
      className={cn(
        "bg-popover text-foreground fixed top-0 left-0 z-40 rounded-md border p-3 text-sm font-normal tracking-normal normal-case shadow-md outline-none",
        at === undefined && "invisible",
        className,
      )}
    >
      {children}
    </div>
  )
}
