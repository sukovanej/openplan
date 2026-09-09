import { Check } from "lucide-react"
import { type KeyboardEvent, useEffect, useId, useRef, useState } from "react"

import type { Status } from "@openplan/api-client"
import { cn, Row } from "@openplan/ui"

import { STATUSES, statusIcon, statusLabel, statusMark } from "./status"

// The list holds the focus, so the keys it answers reach it rather than the page behind it, and
// `data-keys-ignore` keeps the app's own single-key bindings off them while it is open.
export function StatusMenu({
  current,
  onPick,
  onClose,
  className,
}: {
  current: Status | undefined
  onPick: (status: Status) => void
  onClose: () => void
  className?: string
}) {
  const list = useRef<HTMLUListElement>(null)
  const listId = useId()
  const [active, setActive] = useState(() => Math.max(current === undefined ? 0 : STATUSES.indexOf(current), 0))

  useEffect(() => list.current?.focus(), [])

  const move = (delta: number) => setActive((at) => (at + delta + STATUSES.length) % STATUSES.length)

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
        onPick(STATUSES[active])
        break
      case "Escape":
        event.preventDefault()
        onClose()
        break
    }
  }

  return (
    <ul
      ref={list}
      tabIndex={-1}
      role="listbox"
      aria-label="Status"
      aria-activedescendant={`${listId}-${active}`}
      data-keys-ignore
      onKeyDown={onKeyDown}
      // The header it opens under sets its own case, weight and tracking, and the menu is not a
      // header, so it states the whole of its own type rather than inheriting any of that.
      className={cn(
        "bg-popover text-foreground w-44 rounded-md border p-1 text-sm font-normal tracking-normal normal-case shadow-md outline-none",
        className,
      )}
    >
      {STATUSES.map((status, index) => {
        const Icon = statusIcon(status)
        return (
          <Row
            key={status}
            as="li"
            id={`${listId}-${index}`}
            variant="option"
            role="option"
            aria-selected={index === active}
            active={index === active}
            onMouseMove={() => setActive(index)}
            onMouseDown={(event) => {
              event.preventDefault()
              onPick(status)
            }}
            className="cursor-pointer"
          >
            <Icon className={cn("size-4 shrink-0", statusMark(status))} aria-hidden />
            <span className="grow">{statusLabel(status)}</span>
            {status === current && <Check className="text-muted-foreground size-3.5 shrink-0" aria-label="Current" />}
          </Row>
        )
      })}
    </ul>
  )
}
