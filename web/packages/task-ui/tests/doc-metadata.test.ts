import { describe, expect, it } from "vitest"

import type { DocFields, DocMetadata } from "@openplan/api-client"

import { docConflictedFields, docParentOf, docProblems } from "../src/doc-metadata"

const parentConflict = {
  kind: "conflict",
  value: "reference",
  sides: [
    { label: "Ann", value: "guides" },
    { label: "Ben", value: "reference" },
  ],
} as const

const doc = (parent: DocFields["parent"]): DocMetadata => ({
  created: "2026-07-31T14:08:22Z",
  parent,
})

describe("docConflictedFields", () => {
  it("names the parent when two sides of a sync set it differently", () => {
    expect(docConflictedFields(doc(parentConflict))).toEqual(["parent"])
  })

  it("names nothing for a doc without conflicts, or with frontmatter that did not parse", () => {
    expect(docConflictedFields(doc("guides"))).toEqual([])
    expect(docConflictedFields({ kind: "error", message: "no frontmatter" })).toEqual([])
  })
})

describe("a doc parent in conflict", () => {
  it("reads as the published version", () => {
    expect(docParentOf(doc(parentConflict))).toBe("reference")
  })

  it("is not a problem with the field", () => {
    expect(docProblems(doc(parentConflict))).toEqual([])
  })
})
