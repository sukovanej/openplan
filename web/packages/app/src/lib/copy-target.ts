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

export function copyTargetRow(
  hoveredRow: string | undefined,
  focusedRow: string | undefined,
  routeRow: string | undefined,
): string | undefined {
  return hoveredRow ?? focusedRow ?? routeRow
}
