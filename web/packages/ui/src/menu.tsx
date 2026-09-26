import { type KeyboardEvent, type ReactNode, useEffect, useId, useRef, useState } from "react"

import { cn } from "./cn"
import { Row } from "./row"

export interface MenuItem {
  readonly key: string
  readonly content: ReactNode
  readonly shortcut?: string
}

// The list holds the focus, so the keys it answers reach it rather than the page behind it, and
// `data-keys-ignore` keeps the app's own single-key bindings off them while it is open.
export function Menu({
  label,
  items,
  initial,
  onPick,
  onClose,
  indexOfShortcut,
  className,
}: {
  label: string
  items: ReadonlyArray<MenuItem>
  initial: number
  onPick: (index: number) => void
  onClose: () => void
  indexOfShortcut?: (key: string) => number | undefined
  className?: string
}) {
  const list = useRef<HTMLUListElement>(null)
  const listId = useId()
  const [active, setActive] = useState(() => Math.max(initial, 0))

  useEffect(() => list.current?.focus(), [])

  const move = (delta: number) => setActive((at) => (at + delta + items.length) % items.length)

  const onKeyDown = (event: KeyboardEvent) => {
    switch (event.key) {
      case "ArrowDown":
      case "j":
        event.preventDefault()
        move(1)
        break
      case "ArrowUp":
      case "k":
        event.preventDefault()
        move(-1)
        break
      case "Enter":
      case " ":
        event.preventDefault()
        onPick(active)
        break
      case "Escape":
        event.preventDefault()
        onClose()
        break
      default: {
        if (event.ctrlKey || event.metaKey || event.altKey) break
        const index = indexOfShortcut?.(event.key)
        if (index === undefined) break
        event.preventDefault()
        onPick(index)
      }
    }
  }

  return (
    <ul
      ref={list}
      tabIndex={-1}
      role="listbox"
      aria-label={label}
      aria-activedescendant={`${listId}-${active}`}
      data-keys-ignore
      onKeyDown={onKeyDown}
      // The header it opens under sets its own case, weight and tracking, and the menu is not a
      // header, so it states the whole of its own type rather than inheriting any of that.
      className={cn(
        "bg-popover text-foreground w-48 rounded-md border p-1 text-sm font-normal tracking-normal normal-case shadow-md outline-none",
        className,
      )}
    >
      {items.map((item, index) => (
        <Row
          key={item.key}
          as="li"
          id={`${listId}-${index}`}
          variant="option"
          role="option"
          aria-selected={index === active}
          aria-keyshortcuts={item.shortcut}
          active={index === active}
          onMouseMove={() => setActive(index)}
          onMouseDown={(event) => {
            event.preventDefault()
            onPick(index)
          }}
          className="cursor-pointer"
        >
          {item.content}
        </Row>
      ))}
    </ul>
  )
}
