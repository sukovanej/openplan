import { useEffect, useRef } from "react"

// The row whose status menu the keyboard asks for. A task detail can show one task under two of its
// lists, so the place answers as well as the task: only the row at hand opens. `-1` is the page's
// own task, which sits on no row.
export const NO_ROW = -1

export interface StatusTarget {
  readonly project: string
  readonly id: string
  readonly at: number
}

export const sameTarget = (a: StatusTarget, b: StatusTarget): boolean =>
  a.project === b.project && a.id === b.id && a.at === b.at

class StatusRequests {
  private readonly listeners = new Set<(target: StatusTarget) => void>()

  readonly on = (listener: (target: StatusTarget) => void): (() => void) => {
    this.listeners.add(listener)
    return () => {
      this.listeners.delete(listener)
    }
  }

  readonly emit = (target: StatusTarget): void => {
    for (const listener of this.listeners) listener(target)
  }
}

export const statusRequests = new StatusRequests()

export function useStatusRequest(target: StatusTarget, open: () => void): void {
  const latest = useRef({ target, open })
  latest.current = { target, open }
  useEffect(
    () =>
      statusRequests.on((asked) => {
        if (sameTarget(asked, latest.current.target)) latest.current.open()
      }),
    [],
  )
}
