export type RowGuides = {
  readonly columns: ReadonlyArray<boolean>
  readonly opensChildren: boolean
}

// Column j of a row at depth d carries the branch of its ancestor at depth j + 1, and holds `true`
// while that ancestor still has a sibling to come. The row's own column — the last one — reads the
// same way: `true` draws a tee, `false` the elbow that closes the group.
export function treeGuides(rows: ReadonlyArray<{ readonly depth: number }>): Array<RowGuides> {
  const guides: Array<RowGuides> = []
  const pending: Array<boolean> = []
  for (let i = rows.length - 1; i >= 0; i--) {
    const { depth } = rows[i]
    guides[i] = {
      columns: Array.from({ length: depth }, (_, column) => pending[column + 1] === true),
      opensChildren: (rows[i + 1]?.depth ?? 0) > depth,
    }
    // A row at this depth closes every subtree below it.
    pending.length = depth
    pending[depth] = true
  }
  return guides
}
