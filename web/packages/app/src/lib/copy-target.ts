// Rows are named by their task's path throughout, so a key that repeats across projects still
// names one row.
let hovered: string | undefined

export const hoveredRow = {
  enter: (row: string): void => {
    hovered = row
  },
  // Guards against a leave that names a row the store has already moved past, whatever order the
  // leaving and entering rows report in.
  leave: (row: string): void => {
    if (hovered === row) hovered = undefined
  },
  clear: (): void => {
    hovered = undefined
  },
  // A row that goes away under a still pointer — deleted, reparented, refreshed out of the list —
  // fires no mouseleave, so a hover counts only while its row is one of those on screen.
  among: (rendered: ReadonlyArray<string>): string | undefined =>
    hovered !== undefined && rendered.includes(hovered) ? hovered : undefined,
}

export function copyTargetRow(
  hoveredRow: string | undefined,
  focusedRow: string | undefined,
  routeRow: string | undefined,
): string | undefined {
  return hoveredRow ?? focusedRow ?? routeRow
}
