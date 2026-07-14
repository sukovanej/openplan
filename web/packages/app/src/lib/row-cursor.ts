import { useLayoutEffect, useSyncExternalStore } from "react"

export interface CursorState {
  readonly ids: ReadonlyArray<string>
  readonly index: number
}

export const emptyCursor: CursorState = { ids: [], index: -1 }

export function clampIndex(index: number, count: number): number {
  if (count <= 0) return -1
  if (index < 0) return 0
  if (index >= count) return count - 1
  return index
}

function sameIds(a: ReadonlyArray<string>, b: ReadonlyArray<string>): boolean {
  if (a === b) return true
  if (a.length !== b.length) return false
  for (let i = 0; i < a.length; i++) {
    if (a[i] !== b[i]) return false
  }
  return true
}

export function withRows(state: CursorState, ids: ReadonlyArray<string>): CursorState {
  if (sameIds(state.ids, ids)) return state
  return { ids, index: -1 }
}

export function moved(state: CursorState, delta: number): CursorState {
  const index = clampIndex(state.index + delta, state.ids.length)
  return index === state.index ? state : { ids: state.ids, index }
}

export function focused(state: CursorState, index: number): CursorState {
  const index_ = clampIndex(index, state.ids.length)
  return index_ === state.index ? state : { ids: state.ids, index: index_ }
}

export function focusedId(state: CursorState): string | undefined {
  return state.index < 0 ? undefined : state.ids[state.index]
}

class RowCursorStore {
  private state: CursorState = emptyCursor
  private readonly listeners = new Set<() => void>()

  readonly subscribe = (listener: () => void): () => void => {
    this.listeners.add(listener)
    return () => {
      this.listeners.delete(listener)
    }
  }

  readonly getSnapshot = (): CursorState => this.state

  readonly setRows = (ids: ReadonlyArray<string>): void => this.commit(withRows(this.state, ids))
  readonly moveBy = (delta: number): void => this.commit(moved(this.state, delta))
  readonly focus = (index: number): void => this.commit(focused(this.state, index))

  private commit(next: CursorState): void {
    if (next === this.state) return
    this.state = next
    for (const listener of this.listeners) listener()
  }
}

export const rowCursor = new RowCursorStore()

export function useRowCursor(ids: ReadonlyArray<string>): CursorState {
  const state = useSyncExternalStore(rowCursor.subscribe, rowCursor.getSnapshot)
  useLayoutEffect(() => {
    rowCursor.setRows(ids)
  }, [ids])
  return state
}
