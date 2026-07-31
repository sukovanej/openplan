import { type ReactNode, useEffect, useId, useLayoutEffect, useRef, useState } from "react"

import { cn } from "./cn"

const GAP = 6
const HOVER_DELAY = 300

export function Tooltip({
  content,
  className,
  children,
}: {
  content: ReactNode
  className?: string
  children: ReactNode
}) {
  const anchor = useRef<HTMLSpanElement>(null)
  const pointed = useRef(false)
  const bubble = useRef<HTMLDivElement>(null)
  const delay = useRef<number>(undefined)
  const [shown, setShown] = useState(false)
  const [at, setAt] = useState<{ left: number; top: number }>()
  const id = useId()

  useLayoutEffect(() => {
    if (!shown) return
    const place = () => {
      const from = anchor.current?.getBoundingClientRect()
      const box = bubble.current?.getBoundingClientRect()
      if (from === undefined || box === undefined) return
      const above = from.top - box.height - GAP
      setAt({
        left: Math.min(Math.max(GAP, from.left + from.width / 2 - box.width / 2), window.innerWidth - box.width - GAP),
        top: above < GAP ? from.bottom + GAP : above,
      })
    }
    place()
    // The bubble hangs off the viewport rather than the row, which is what keeps a scrolling panel
    // from clipping it — so whatever moves the element it points at has to move it too.
    window.addEventListener("scroll", place, true)
    window.addEventListener("resize", place)
    return () => {
      window.removeEventListener("scroll", place, true)
      window.removeEventListener("resize", place)
    }
  }, [shown, content])

  useEffect(() => () => window.clearTimeout(delay.current), [])

  const hide = () => {
    window.clearTimeout(delay.current)
    setShown(false)
    setAt(undefined)
  }

  return (
    <span
      ref={anchor}
      aria-describedby={shown ? id : undefined}
      className={cn("inline-flex", className)}
      // A pointer sweeping a list crosses elements it is not asking about; a keyboard landing on one
      // is asking.
      onPointerEnter={() => {
        delay.current = window.setTimeout(() => setShown(true), HOVER_DELAY)
      }}
      onPointerLeave={hide}
      onPointerDown={() => {
        pointed.current = true
      }}
      onFocus={() => {
        if (!pointed.current) setShown(true)
      }}
      onBlur={() => {
        pointed.current = false
        hide()
      }}
    >
      {children}
      {shown && (
        <div
          ref={bubble}
          id={id}
          role="tooltip"
          style={at}
          className={cn(
            "bg-background text-foreground/90 pointer-events-none fixed top-0 left-0 z-50 max-w-xs rounded-md border px-2 py-1 text-xs shadow-md",
            // It spends its first layout at the corner it is measured in.
            at === undefined && "invisible",
          )}
        >
          {content}
        </div>
      )}
    </span>
  )
}
