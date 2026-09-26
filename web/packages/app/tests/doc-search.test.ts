import { describe, expect, it } from "vitest"

import type { DocListItem } from "@openplan/api-client"

import { docMatches } from "../src/lib/doc-search"

function doc(name: string, title = name): DocListItem {
  return {
    project: "one",
    name,
    title,
    metadata: { created: "2026-01-01T00:00:00Z", parent: null },
    updated: "2026-01-01T00:00:00Z",
  } as unknown as DocListItem
}

const docs = [doc("storage", "Storage"), doc("style-guide", "Style guide"), doc("setup", "Local setup")]

const names = (query: string, excluded: ReadonlyArray<string> = []) =>
  docMatches(docs, query, new Set(excluded)).map((match) => match.doc.name)

describe("docMatches", () => {
  it("lists every doc by title for an empty query", () => {
    expect(names("")).toEqual(["setup", "storage", "style-guide"])
  })

  it("ranks a match at the start of the title first", () => {
    expect(names("st")).toEqual(["storage", "style-guide", "setup"])
  })

  it("leaves out the excluded docs", () => {
    expect(names("", ["storage"])).toEqual(["setup", "style-guide"])
  })

  it("matches the name when the doc has no title", () => {
    expect(docMatches([doc("roadmap", "")], "road", new Set()).map((match) => match.indices)).toEqual([[0, 1, 2, 3]])
  })

  it("shows at most eight docs", () => {
    const many = Array.from({ length: 12 }, (_, at) => doc(`doc-${at}`))
    expect(docMatches(many, "", new Set())).toHaveLength(8)
  })
})
