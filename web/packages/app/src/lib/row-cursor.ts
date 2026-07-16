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

export function cleared(state: CursorState): CursorState {
  return state.index === -1 ? state : { ids: state.ids, index: -1 }
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
  readonly clear = (): void => this.commit(cleared(this.state))

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

// A cursor whose focused row is remembered per key, so navigating parent → child → back leaves the
// parent's subtask still highlighted. A key never visited starts unselected, unlike a bounds clamp.
class KeyedRowCursor {
  private activeKey = ""
  private state: CursorState = emptyCursor
  private readonly saved = new Map<string, number>()
  private readonly listeners = new Set<() => void>()

  readonly subscribe = (listener: () => void): () => void => {
    this.listeners.add(listener)
    return () => {
      this.listeners.delete(listener)
    }
  }

  readonly getSnapshot = (): CursorState => this.state

  readonly activate = (key: string, ids: ReadonlyArray<string>): void => {
    const prior = this.saved.get(key)
    const index = prior === undefined || ids.length === 0 ? -1 : Math.min(prior, ids.length - 1)
    this.activeKey = key
    this.commit({ ids, index })
  }

  readonly moveBy = (delta: number): void => this.commit(moved(this.state, delta))
  readonly focus = (index: number): void => this.commit(focused(this.state, index))
  readonly clear = (): void => this.commit(cleared(this.state))

  private commit(next: CursorState): void {
    if (next === this.state) return
    this.state = next
    if (this.activeKey !== "") this.saved.set(this.activeKey, next.index)
    for (const listener of this.listeners) listener()
  }
}

export const subtaskCursor = new KeyedRowCursor()

export function useSubtaskCursor(key: string, ids: ReadonlyArray<string>): CursorState {
  const state = useSyncExternalStore(subtaskCursor.subscribe, subtaskCursor.getSnapshot)
  useLayoutEffect(() => {
    subtaskCursor.activate(key, ids)
  }, [key, ids])
  return state
}
