import type { Dispatch, SetStateAction } from "react"

import type { OverlayName } from "./keys/types"

type Listener = Dispatch<SetStateAction<OverlayName | null>>

// The way a button or a palette command opens an overlay that the keyboard layer owns.
class OverlayRequests {
  private readonly listeners = new Set<Listener>()

  readonly on = (listener: Listener): (() => void) => {
    this.listeners.add(listener)
    return () => {
      this.listeners.delete(listener)
    }
  }

  readonly open = (name: OverlayName): void => {
    for (const listener of this.listeners) listener(name)
  }

  readonly toggle = (name: OverlayName): void => {
    for (const listener of this.listeners) listener((open) => (open === name ? null : name))
  }
}

export const overlayRequests = new OverlayRequests()
