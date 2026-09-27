import { type ReactNode, useId, useLayoutEffect, useRef, useState } from "react"

import { cn } from "./cn"

const GAP = 6

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

  const hide = () => {
    setShown(false)
    setAt(undefined)
  }

  return (
    <span
      ref={anchor}
      aria-describedby={shown ? id : undefined}
      className={cn("inline-flex", className)}
      onPointerEnter={() => setShown(true)}
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
