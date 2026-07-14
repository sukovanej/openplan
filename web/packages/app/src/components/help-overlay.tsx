import { X } from "lucide-react"
import type * as React from "react"
import { useEffect, useRef } from "react"

import { bindings, type HelpEntry, helpGroups } from "@/lib/keys"
import { cn } from "@/lib/utils"

const KEY_LABELS: Record<string, string> = {
  Escape: "Esc",
  Enter: "↵",
  ArrowUp: "↑",
  ArrowDown: "↓",
  " ": "Space",
}

function keyLabel(token: string): string {
  return KEY_LABELS[token] ?? (token.length === 1 ? token.toUpperCase() : token)
}

const FOCUSABLE = "button, [href], [tabindex]:not([tabindex=\"-1\"])"

const GROUPS = helpGroups(bindings)

export function HelpOverlay({ open, onClose }: { open: boolean; onClose: () => void }) {
  const dialog = useRef<HTMLDivElement>(null)
  const restore = useRef<HTMLElement | null>(null)

  useEffect(() => {
    if (!open) return
    restore.current = document.activeElement instanceof HTMLElement ? document.activeElement : null
    dialog.current?.focus()
    return () => restore.current?.focus()
  }, [open])

  if (!open) return null

  const trap = (event: React.KeyboardEvent) => {
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
        aria-label="Keyboard shortcuts"
        tabIndex={-1}
        onClick={(event) => event.stopPropagation()}
        onKeyDown={trap}
        className="bg-background w-full max-w-md rounded-xl border p-6 shadow-lg focus:outline-none"
      >
        <div className="mb-5 flex items-center justify-between">
          <h2 className="text-lg font-semibold tracking-tight">Keyboard shortcuts</h2>
          <button
            type="button"
            aria-label="Close"
            onClick={onClose}
            className="text-muted-foreground hover:text-foreground focus-visible:ring-ring inline-flex size-7 items-center justify-center rounded-md transition-colors focus-visible:ring-2 focus-visible:outline-none"
          >
            <X className="size-4" aria-hidden="true" />
          </button>
        </div>
        <div className="space-y-5">
          {GROUPS.map((group) => (
            <section key={group.name}>
              <h3 className="text-muted-foreground mb-2 text-xs font-medium tracking-wide uppercase">
                {group.name}
              </h3>
              <ul className="space-y-1.5">
                {group.entries.map((entry) => <HelpRow key={entry.id} entry={entry} />)}
              </ul>
            </section>
          ))}
        </div>
      </div>
    </div>
  )
}

function HelpRow({ entry }: { entry: HelpEntry }) {
  return (
    <li className="flex items-center justify-between gap-4">
      <span className="text-foreground/90 text-sm">{entry.label}</span>
      <span className="flex items-center gap-1">
        {entry.keys.map((token, index) => (
          <kbd
            key={index}
            className={cn(
              "bg-muted text-muted-foreground inline-flex h-6 min-w-6 items-center justify-center rounded border px-1.5 text-xs font-medium",
            )}
          >
            {keyLabel(token)}
          </kbd>
        ))}
      </span>
    </li>
  )
}
