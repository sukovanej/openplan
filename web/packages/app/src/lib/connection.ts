import { useSyncExternalStore } from "react"

// `updating` is a daemon that starts again on a new release. `outdated` is a page that still runs the
// web app of the release before, because a reload would drop what the user typed.
export type ConnectionState = "connecting" | "live" | "reconnecting" | "stopped" | "updating" | "outdated"

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
