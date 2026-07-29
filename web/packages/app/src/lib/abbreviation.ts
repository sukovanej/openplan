import { useSyncExternalStore } from "react"

// The store's key prefix, which the daemon owns and can change while the page is open. Held apart
// from the task queries because it is not task data: it decides how every id already on screen reads,
// so a `[[…]]` that names this store has to be told from one written for another.
class AbbreviationStore {
  private value: string | undefined
  private readonly listeners = new Set<() => void>()

  readonly subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener)
    return () => {
      this.listeners.delete(listener)
    }
  }

  readonly getSnapshot = (): string | undefined => this.value

  readonly set = (next: string | undefined): void => {
    if (next === this.value) return
    this.value = next
    for (const listener of this.listeners) listener()
  }
}

export const abbreviationStore = new AbbreviationStore()

export function useAbbreviation(): string | undefined {
  return useSyncExternalStore(abbreviationStore.subscribe, abbreviationStore.getSnapshot)
}
