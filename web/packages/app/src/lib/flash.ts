import { useSyncExternalStore } from "react"

export type FlashTone = "ok" | "error"

export interface FlashMessage {
  readonly text: string
  readonly tone: FlashTone
}

const VISIBLE_MS = 1600

class FlashStore {
  private message: FlashMessage | undefined
  private timer: ReturnType<typeof setTimeout> | undefined
  private readonly listeners = new Set<() => void>()

  readonly subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener)
    return () => {
      this.listeners.delete(listener)
    }
  }

  readonly getSnapshot = (): FlashMessage | undefined => this.message

  readonly show = (text: string, tone: FlashTone): void => {
    if (this.timer !== undefined) clearTimeout(this.timer)
    this.timer = setTimeout(this.clear, VISIBLE_MS)
    this.set({ text, tone })
  }

  readonly clear = (): void => {
    if (this.timer !== undefined) {
      clearTimeout(this.timer)
      this.timer = undefined
    }
    this.set(undefined)
  }

  private set(next: FlashMessage | undefined): void {
    if (next === this.message) return
    this.message = next
    for (const listener of this.listeners) listener()
  }
}

export const flash = new FlashStore()

export function useFlash(): FlashMessage | undefined {
  return useSyncExternalStore(flash.subscribe, flash.getSnapshot)
}
