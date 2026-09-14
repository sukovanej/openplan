import { type ComponentProps, type KeyboardEvent, type ReactNode, useEffect, useRef } from "react"
import { createPortal } from "react-dom"

import { cn } from "./cn"

const FOCUSABLE = 'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'

// The modal renders under <body>, so the prose, stacking and scroll of the content that opens it
// stay off it.
export function Modal({
  open,
  onClose,
  label,
  className,
  children,
  ...attributes
}: {
  open: boolean
  onClose: () => void
  label: string
  className?: string
  children: ReactNode
} & Omit<ComponentProps<"div">, "className" | "children">) {
  const dialog = useRef<HTMLDivElement>(null)
  const restore = useRef<HTMLElement | null>(null)

  useEffect(() => {
    if (!open) return
    restore.current = document.activeElement instanceof HTMLElement ? document.activeElement : null
    dialog.current?.focus()
    return () => restore.current?.focus()
  }, [open])

  if (!open) return null

  const trap = (event: KeyboardEvent) => {
    if (event.key !== "Tab" || dialog.current === null) return
    const focusable = [...dialog.current.querySelectorAll<HTMLElement>(FOCUSABLE)]
    if (focusable.length === 0) {
      event.preventDefault()
      return
    }
    const first = focusable[0]
    const last = focusable[focusable.length - 1]
    const active = document.activeElement
    if (event.shiftKey && (active === first || active === dialog.current)) {
      event.preventDefault()
      last.focus()
    } else if (!event.shiftKey && active === last) {
      event.preventDefault()
      first.focus()
    }
  }

  // The app's key dispatcher skips a key that is already handled, so Esc closes the modal and not
  // the page behind it.
  const onKeyDown = (event: KeyboardEvent) => {
    if (event.key === "Escape") {
      event.preventDefault()
      onClose()
    } else {
      trap(event)
    }
  }

  return createPortal(
    <div
      role="presentation"
      onClick={onClose}
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/20 p-4 backdrop-blur-[1px]"
    >
      <div
        {...attributes}
        ref={dialog}
        role="dialog"
        aria-modal="true"
        aria-label={label}
        tabIndex={-1}
        onClick={(event) => event.stopPropagation()}
        onKeyDown={onKeyDown}
        className={cn("focus:outline-none", className)}
      >
        {children}
      </div>
    </div>,
    document.body,
  )
}
