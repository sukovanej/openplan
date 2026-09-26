import { describe, expect, it } from "vitest"

import type { DocListItem } from "@openplan/api-client"

import { docTree } from "../src/lib/doc-tree"

function doc(name: string, parent?: string, project = "one"): DocListItem {
  return {
    project,
    name,
    title: name,
    metadata: { created: "2026-01-01T00:00:00Z", parent: parent ?? null },
    updated: "2026-01-01T00:00:00Z",
  } as unknown as DocListItem
}

const shape = (docs: ReadonlyArray<DocListItem>) =>
  docTree(docs).map((row) => `${"-".repeat(row.depth)}${row.doc.name}`)

describe("docTree", () => {
  it("reads a parent the file named badly as no parent, and says so through the field", () => {
    const broken = { ...doc("storage"), metadata: { kind: "error", message: "bad yaml" } } as unknown as DocListItem
    expect(docTree([doc("architecture"), broken]).map((row) => row.depth)).toEqual([0, 0])
  })

  it("puts each doc under its parent, in the order the list gives them", () => {
    expect(shape([doc("architecture"), doc("blobs", "storage"), doc("storage", "architecture"), doc("style")])).toEqual(
      ["architecture", "-storage", "--blobs", "style"],
    )
  })

  it("keeps a doc whose parent is not in the list at the top level", () => {
    expect(shape([doc("storage", "architecture")])).toEqual(["storage"])
  })

  it("nests only within one project, because a parent names a doc in its own store", () => {
    const rows = docTree([doc("architecture", undefined, "one"), doc("storage", "architecture", "two")])
    expect(rows.map((row) => row.depth)).toEqual([0, 0])
  })

  it("lists every doc once when the files carry a cycle", () => {
    expect(shape([doc("a", "b"), doc("b", "a")]).sort()).toEqual(["-b", "a"])
  })
})
