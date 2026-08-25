import { useLayoutEffect, useSyncExternalStore } from "react"

// A row is named by its task's path, which carries the project: a key alone repeats across stores,
// and the path is also what opening the row navigates to.
export interface CursorState {
  readonly rows: ReadonlyArray<string>
  readonly index: number
}

export const emptyCursor: CursorState = { rows: [], index: -1 }

export function clampIndex(index: number, count: number): number {
  if (count <= 0) return -1
  if (index < 0) return 0
  if (index >= count) return count - 1
  return index
}

function sameRows(a: ReadonlyArray<string>, b: ReadonlyArray<string>): boolean {
  if (a === b) return true
  if (a.length !== b.length) return false
  for (let i = 0; i < a.length; i++) {
    if (a[i] !== b[i]) return false
  }
  return true
}

export function withRows(state: CursorState, rows: ReadonlyArray<string>): CursorState {
  if (sameRows(state.rows, rows)) return state
  return { rows, index: -1 }
}

export function moved(state: CursorState, delta: number, from?: string): CursorState {
  const origin = state.index === -1 && from !== undefined ? state.rows.indexOf(from) : state.index
  const index = clampIndex(origin + delta, state.rows.length)
  return index === state.index ? state : { rows: state.rows, index }
}

export function focused(state: CursorState, index: number): CursorState {
  const index_ = clampIndex(index, state.rows.length)
  return index_ === state.index ? state : { rows: state.rows, index: index_ }
}

export function cleared(state: CursorState): CursorState {
  return state.index === -1 ? state : { rows: state.rows, index: -1 }
}

export function focusedRow(state: CursorState): string | undefined {
  return state.index < 0 ? undefined : state.rows[state.index]
}

class RowCursorStore {
  private state: CursorState = emptyCursor
  private readonly listeners = new Set<() => void>()

  readonly subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener)
    return () => {
      this.listeners.delete(listener)
    }
  }

  readonly getSnapshot = (): CursorState => this.state

  readonly setRows = (rows: ReadonlyArray<string>): void => this.commit(withRows(this.state, rows))
  readonly moveBy = (delta: number, from?: string): void => this.commit(moved(this.state, delta, from))
  readonly focus = (index: number): void => this.commit(focused(this.state, index))
  readonly clear = (): void => this.commit(cleared(this.state))

  private commit(next: CursorState): void {
    if (next === this.state) return
    this.state = next
    for (const listener of this.listeners) listener()
  }
}

export const rowCursor = new RowCursorStore()

export function useRowCursor(rows: ReadonlyArray<string>): CursorState {
  const state = useSyncExternalStore(rowCursor.subscribe, rowCursor.getSnapshot)
  useLayoutEffect(() => {
    rowCursor.setRows(rows)
  }, [rows])
  return state
}

// A cursor whose focused row is remembered per key, so navigating parent → child → back leaves the
// parent's row still highlighted. A key never visited starts unselected, unlike a bounds clamp.
class KeyedRowCursor {
  private activeKey = ""
  private state: CursorState = emptyCursor
  private readonly saved = new Map<string, number>()
  private readonly listeners = new Set<() => void>()

  readonly subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener)
    return () => {
      this.listeners.delete(listener)
    }
  }

  readonly getSnapshot = (): CursorState => this.state

  readonly activate = (key: string, rows: ReadonlyArray<string>): void => {
    const prior = this.saved.get(key)
    const index = prior === undefined || rows.length === 0 ? -1 : Math.min(prior, rows.length - 1)
    this.activeKey = key
    this.commit({ rows, index })
  }

  readonly moveBy = (delta: number, from?: string): void => this.commit(moved(this.state, delta, from))
  readonly focus = (index: number): void => this.commit(focused(this.state, index))
  readonly clear = (): void => this.commit(cleared(this.state))

  private commit(next: CursorState): void {
    if (next === this.state) return
    this.state = next
    if (this.activeKey !== "") this.saved.set(this.activeKey, next.index)
    for (const listener of this.listeners) listener()
  }
}

export const detailCursor = new KeyedRowCursor()

export function useDetailCursor(key: string, rows: ReadonlyArray<string>): CursorState {
  const state = useSyncExternalStore(detailCursor.subscribe, detailCursor.getSnapshot)
  useLayoutEffect(() => {
    detailCursor.activate(key, rows)
  }, [key, rows])
  return state
}
