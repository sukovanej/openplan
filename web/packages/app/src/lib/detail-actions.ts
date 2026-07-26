import { useEffect, useRef } from "react"

export type DetailAction = "edit-parent" | "add-subtask" | "go-parent"

export type EscapeOutcome = "clear-selection" | "back" | "to-list"

export function escapeOutcome(hasSelection: boolean, canGoBack: boolean): EscapeOutcome {
  if (hasSelection) return "clear-selection"
  return canGoBack ? "back" : "to-list"
}

class DetailActionsBus {
  private readonly listeners = new Map<DetailAction, Set<() => void>>()

  readonly on = (action: DetailAction, listener: () => void): (() => void) => {
    const set = this.listeners.get(action) ?? new Set<() => void>()
    set.add(listener)
    this.listeners.set(action, set)
    return () => set.delete(listener)
  }

  readonly emit = (action: DetailAction): void => {
    for (const listener of this.listeners.get(action) ?? []) listener()
  }
}

export const detailActions = new DetailActionsBus()

export function useDetailAction(action: DetailAction, handler: () => void): void {
  const latest = useRef(handler)
  latest.current = handler
  useEffect(() => detailActions.on(action, () => latest.current()), [action])
}
