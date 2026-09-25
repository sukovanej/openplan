import { useSyncExternalStore } from "react"

import { overlayRequests } from "./overlay-requests"

export interface Selected {
  readonly project: string
  readonly id: string
}

// The session the prompt bar shows, or a new prompt when none. It lives outside React so the keys,
// the header control, and the bar agree, and so it outlasts the page under the bar. The project
// travels with the id because a session the list has not caught up with yet is still one to show.
class PromptBarStore {
  private selected: Selected | undefined
  private readonly listeners = new Set<() => void>()

  readonly getSnapshot = (): Selected | undefined => this.selected

  readonly subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener)
    return () => {
      this.listeners.delete(listener)
    }
  }

  readonly select = (next: Selected | undefined): void => {
    this.selected = next
    for (const listener of this.listeners) listener()
  }

  readonly startNew = (): void => {
    this.select(undefined)
    overlayRequests.open("prompt")
  }
}

export const promptBar = new PromptBarStore()

export function usePromptBarSelection(): Selected | undefined {
  return useSyncExternalStore(promptBar.subscribe, promptBar.getSnapshot)
}
