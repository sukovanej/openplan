import { describe, expect, it } from "vitest"

import { treeGuides } from "../src/lib/tree-guides"

const depths = (values: Array<number>) => values.map((depth) => ({ depth }))

describe("treeGuides", () => {
  it("gives root rows no columns", () => {
    expect(treeGuides(depths([0, 0]))).toEqual([
      { columns: [], opensChildren: false },
      { columns: [], opensChildren: false },
    ])
  })

  it("marks the row a child hangs from", () => {
    const [parent, child] = treeGuides(depths([0, 1]))
    expect(parent.opensChildren).toBe(true)
    expect(child.opensChildren).toBe(false)
  })

  it("tees every child but the last", () => {
    const [, first, second, last] = treeGuides(depths([0, 1, 1, 1]))
    expect([first.columns, second.columns, last.columns]).toEqual([[true], [true], [false]])
  })

  it("carries an ancestor's line down past its nephews", () => {
    // 0 ├ 1 │ ├ 2 │ │ └ 3 (grandchild) │ └ 4 (last child) │ 5 (next root)
    const guides = treeGuides(depths([0, 1, 2, 1, 0]))
    expect(guides.map((row) => row.columns)).toEqual([[], [true], [true, false], [false], []])
  })

  it("drops an ancestor's line once its last child is past", () => {
    const guides = treeGuides(depths([0, 1, 2, 2]))
    expect(guides.map((row) => row.columns)).toEqual([[], [false], [false, true], [false, false]])
  })
})
