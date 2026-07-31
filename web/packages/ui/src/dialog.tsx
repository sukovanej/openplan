import { X } from "lucide-react"
import { type KeyboardEvent, type ReactNode, useEffect, useRef } from "react"

import { Button } from "./button"
import { cn } from "./cn"

const FOCUSABLE = 'button, [href], [tabindex]:not([tabindex="-1"])'

export function Dialog({
  open,
  onClose,
  title,
  className,
  children,
}: {
  open: boolean
  onClose: () => void
  title: string
  className?: string
  children: ReactNode
}) {
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

  return (
    <div
      role="presentation"
      onClick={onClose}
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/20 p-4 backdrop-blur-[1px]"
    >
      <div
        ref={dialog}
        role="dialog"
        aria-modal="true"
        aria-label={title}
        tabIndex={-1}
        onClick={(event) => event.stopPropagation()}
        onKeyDown={trap}
        className={cn("bg-background w-full max-w-md rounded-xl border p-6 shadow-lg focus:outline-none", className)}
      >
        <div className="mb-5 flex items-center justify-between">
          <h2 className="text-lg font-semibold tracking-tight">{title}</h2>
          <Button size="icon" aria-label="Close" onClick={onClose}>
            <X className="size-4" aria-hidden="true" />
          </Button>
        </div>
        {children}
      </div>
    </div>
  )
}
