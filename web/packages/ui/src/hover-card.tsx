import {
  type FocusEvent,
  type KeyboardEvent,
  type PointerEvent,
  type ReactNode,
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
} from "react"

import { cn } from "./cn"
const GAP = 6
// The arrow of the cursor hangs this far below its point.
const CURSOR_GAP = 16

// A keyboard can land on one anchor while the pointer rests on another, and two cards would cover
// each other.
let closeOpen: (() => void) | undefined

// The card is a child of the anchor in the DOM, so a focus inside the card is still inside the
// anchor. It is fixed to the viewport, like the tooltip, so a clipping row cannot cut it.
// `content` mounts only while the card is open.
export function HoverCard({
  label,
  content,
  className,
  cardClassName,
  children,
}: {
  label: string
  content: ReactNode
  className?: string
  cardClassName?: string
  children: ReactNode
}) {
  const anchor = useRef<HTMLDivElement>(null)
  const card = useRef<HTMLDivElement>(null)
  const pointed = useRef(false)
  // A card that a keyboard opened hangs off the anchor and stays until the focus leaves, whatever
  // the pointer does.
  const focused = useRef(false)
  const cursor = useRef<{ x: number; y: number }>(undefined)
  const [open, setOpen] = useState(false)
  const [at, setAt] = useState<{ left: number; top: number }>()
  const id = useId()
  const [hide] = useState(() => {
    const hide = () => {
      if (closeOpen === hide) closeOpen = undefined
      cursor.current = undefined
      setOpen(false)
      setAt(undefined)
    }
    return hide
  })

  const [place] = useState(() => () => {
    const row = anchor.current?.getBoundingClientRect()
    const box = card.current?.getBoundingClientRect()
    if (row === undefined || box === undefined) return
    const from =
      cursor.current === undefined
        ? { left: row.left, top: row.top - GAP, bottom: row.bottom + GAP }
        : {
            left: cursor.current.x + CURSOR_GAP,
            top: cursor.current.y - GAP,
            bottom: cursor.current.y + CURSOR_GAP,
          }
    const above = from.top - box.height
    const next = {
      left: Math.max(GAP, Math.min(from.left, window.innerWidth - box.width - GAP)),
      top: from.bottom + box.height > window.innerHeight - GAP && above >= GAP ? above : from.bottom,
    }
    setAt((current) => (current?.left === next.left && current.top === next.top ? current : next))
  })

  const show = () => {
    if (closeOpen !== hide) closeOpen?.()
    closeOpen = hide
    setOpen(true)
  }

  const follow = (event: PointerEvent) => {
    cursor.current = { x: event.clientX, y: event.clientY }
  }

  useLayoutEffect(() => {
    if (!open) return
    place()
    // The content can load after the card opens, and its new height can move the card above.
    const resized = new ResizeObserver(place)
    if (card.current !== null) resized.observe(card.current)
    window.addEventListener("scroll", place, true)
    window.addEventListener("resize", place)
    return () => {
      resized.disconnect()
      window.removeEventListener("scroll", place, true)
      window.removeEventListener("resize", place)
    }
  }, [open, place])

  useEffect(() => hide, [hide])

  const onBlur = (event: FocusEvent) => {
    pointed.current = false
    if (anchor.current?.contains(event.relatedTarget)) return
    focused.current = false
    hide()
  }

  // The app's key dispatcher skips a key that is already handled, so Esc closes the card and not the
  // page behind it.
  const onKeyDown = (event: KeyboardEvent) => {
    if (event.key !== "Escape" || !open) return
    event.preventDefault()
    focused.current = false
    hide()
  }

  return (
    <div
      ref={anchor}
      tabIndex={0}
      aria-haspopup="dialog"
      aria-expanded={open}
      aria-controls={open ? id : undefined}
      className={cn("focus-visible:ring-ring rounded-sm focus-visible:ring-2 focus-visible:outline-none", className)}
      onPointerEnter={(event) => {
        if (open) return
        follow(event)
        show()
      }}
      onPointerMove={(event) => {
        if (!open || focused.current) return
        follow(event)
        place()
      }}
      onPointerLeave={() => {
        if (open && !focused.current) hide()
      }}
      onPointerDown={() => {
        pointed.current = true
      }}
      // A click focuses the anchor too, and a card that the click kept open would stay after the
      // pointer leaves.
      onFocus={() => {
        if (pointed.current || open) return
        focused.current = true
        show()
      }}
      onBlur={onBlur}
      onKeyDown={onKeyDown}
    >
      {children}
      {open && (
        <div
          ref={card}
          id={id}
          role="dialog"
          aria-label={label}
          // Focus lets a keyboard scroll the card, and the keys it scrolls with are not the app's.
          tabIndex={0}
          data-keys-ignore
          // The anchor can sit in a row that opens on a click, and a click that selects or scrolls
          // in the card is not a click on the row.
          onClick={(event) => event.stopPropagation()}
          style={at}
          className={cn(
            "bg-popover text-foreground fixed top-0 left-0 z-40 overflow-auto rounded-md border text-sm font-normal shadow-md outline-none",
            at === undefined && "invisible",
            cardClassName,
          )}
        >
          {content}
        </div>
      )}
    </div>
  )
}
