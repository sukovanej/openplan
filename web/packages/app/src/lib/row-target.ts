import { type TaskRoute, taskRouteOf } from "@openplan/task-ui"

import type { CursorState } from "./row-cursor"

// Rows are named by their task's path throughout, so a key that repeats across projects still
// names one row. A path alone does not name one *row*, though: a task detail can show the same task
// under two of its lists, so the pointer is tracked by the place it is over as well.
let hovered: { readonly row: string; readonly at: number } | undefined

export const hoveredRow = {
  enter: (row: string, at: number): void => {
    hovered = { row, at }
  },
  // Guards against a leave that names a row the store has already moved past, whatever order the
  // leaving and entering rows report in.
  leave: (row: string, at: number): void => {
    if (hovered?.row === row && hovered.at === at) hovered = undefined
  },
  clear: (): void => {
    hovered = undefined
  },
  // Where the pointer's row sits, or -1. A row that goes away under a still pointer — deleted,
  // reparented, refreshed out of the list — fires no mouseleave, so a hover counts only while its
  // row is still the one rendered at that place.
  place: (rendered: ReadonlyArray<string>): number =>
    hovered !== undefined && rendered[hovered.at] === hovered.row ? hovered.at : -1,
  among: (rendered: ReadonlyArray<string>): string | undefined => {
    const at = hoveredRow.place(rendered)
    return at === -1 ? undefined : rendered[at]
  },
}

// Which row the task at hand sits on, so a surface that draws one row per task can tell which of
// them was asked for. `-1` is the page's own task, which sits on no row.
export interface TaskTarget extends TaskRoute {
  readonly at: number
}

export function taskAtHand(cursor: CursorState, pathname: string): TaskTarget | undefined {
  const pointed = hoveredRow.place(cursor.rows)
  const at = pointed === -1 ? cursor.index : pointed
  const route = taskRouteOf(at === -1 ? pathname : cursor.rows[at])
  return route === undefined ? undefined : { ...route, at }
}
