import { describe, expect, it } from "vitest"

import { fuzzyMatch, fuzzySegments } from "../src/lib/fuzzy"

describe("fuzzyMatch", () => {
  it("matches a subsequence and reports the hit positions", () => {
    const match = fuzzyMatch("brd", "board endpoint")
    expect(match).not.toBeNull()
    expect(match!.indices).toEqual([0, 3, 4])
  })

  it("returns null when a character is missing or out of order", () => {
    expect(fuzzyMatch("xyz", "board")).toBeNull()
    expect(fuzzyMatch("rab", "board")).toBeNull()
  })

  it("matches everything with no highlight for an empty query", () => {
    expect(fuzzyMatch("", "anything")).toEqual({ score: 0, indices: [] })
  })

  it("scores an earlier, tighter match below a later, looser one", () => {
    const tight = fuzzyMatch("bo", "board")!
    const loose = fuzzyMatch("bo", "abstract of")!
    expect(tight.score).toBeLessThan(loose.score)
  })

  it("is case-insensitive", () => {
    expect(fuzzyMatch("BO", "board")).not.toBeNull()
  })
})

describe("fuzzySegments", () => {
  it("splits text into matched and unmatched runs", () => {
    expect(fuzzySegments("board", [0, 1])).toEqual([
      { text: "bo", match: true },
      { text: "ard", match: false },
    ])
  })

  it("treats no indices as a single unmatched run", () => {
    expect(fuzzySegments("board", [])).toEqual([{ text: "board", match: false }])
  })

  it("handles a trailing match run", () => {
    expect(fuzzySegments("abc", [2])).toEqual([
      { text: "ab", match: false },
      { text: "c", match: true },
    ])
  })
})
