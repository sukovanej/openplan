import { useSyncExternalStore } from "react"

export type ConnectionState = "connecting" | "live" | "reconnecting" | "stopped"

class ConnectionStore {
  private state: ConnectionState = "connecting"
  private readonly listeners = new Set<() => void>()

  readonly subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener)
    return () => {
      this.listeners.delete(listener)
    }
  }

  readonly getSnapshot = (): ConnectionState => this.state

  readonly set = (state: ConnectionState): void => {
    if (state === this.state) return
    this.state = state
    for (const listener of this.listeners) listener()
  }
}

export const connectionStore = new ConnectionStore()

export function useConnection(): ConnectionState {
  return useSyncExternalStore(connectionStore.subscribe, connectionStore.getSnapshot)
}
