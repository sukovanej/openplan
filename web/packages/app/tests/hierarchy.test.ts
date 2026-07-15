import { describe, expect, it } from "vitest"

import type { TaskListItem } from "../src/lib/api"
import { childrenOf, forestRows, siblingCompare } from "../src/lib/hierarchy"

function task(id: string, parent?: string, rank?: string): TaskListItem {
  return {
    id,
    title: id,
    status: "todo",
    parent,
    rank,
    headline: "main",
    branches: [],
  }
}

describe("siblingCompare", () => {
  it("orders ranked before unranked, ties by id", () => {
    const items = [task("z"), task("a", undefined, "t"), task("b", undefined, "m"), task("y")]
    const ids = items.sort(siblingCompare).map((t) => t.id)
    expect(ids).toEqual(["b", "a", "y", "z"])
  })
})

describe("childrenOf", () => {
  it("returns only direct children, in rank order", () => {
    const items = [
      task("root"),
      task("b", "root", "t"),
      task("a", "root", "m"),
      task("grandchild", "a", "m"),
    ]
    expect(childrenOf(items, "root").map((t) => t.id)).toEqual(["a", "b"])
    expect(childrenOf(items, "a").map((t) => t.id)).toEqual(["grandchild"])
  })
})

describe("forestRows", () => {
  it("pre-orders each subtree with increasing depth", () => {
    const items = [
      task("root", undefined, "m"),
      task("child", "root", "m"),
      task("grandchild", "child", "m"),
      task("sibling", "root", "t"),
    ]
    expect(forestRows(items).map((r) => [r.task.id, r.depth])).toEqual([
      ["root", 0],
      ["child", 1],
      ["grandchild", 2],
      ["sibling", 1],
    ])
  })

  it("treats a dangling parent as a root", () => {
    const items = [task("orphan", "missing", "m")]
    expect(forestRows(items).map((r) => r.task.id)).toEqual(["orphan"])
  })

  it("does not hang on a parent cycle", () => {
    const items = [task("x", "y", "m"), task("y", "x", "m")]
    const ids = forestRows(items).map((r) => r.task.id)
    expect(ids).toHaveLength(2)
    expect(new Set(ids)).toEqual(new Set(["x", "y"]))
  })
})
