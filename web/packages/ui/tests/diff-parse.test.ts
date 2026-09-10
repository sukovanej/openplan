import { describe, expect, it } from "vitest"

import { type DiffLine, parseDiff } from "../src/diff-parse"

const text = (line: DiffLine) => line.spans.map((span) => span.text).join("")
const changed = (line: DiffLine) => line.spans.filter((span) => span.changed).map((span) => span.text)

const modified = [
  "diff --git a/.plan/tasks/00001-t.md b/.plan/tasks/00001-t.md",
  "index 1111111..2222222 100644",
  "--- a/.plan/tasks/00001-t.md",
  "+++ b/.plan/tasks/00001-t.md",
  "@@ -3,4 +3,4 @@ status: todo",
  " # T",
  " ",
  "-The old body.",
  "+The new body.",
  "",
].join("\n")

describe("parseDiff", () => {
  it("drops the file header and keeps the hunk ranges", () => {
    const hunks = parseDiff(modified)
    expect(hunks).toHaveLength(1)
    expect(hunks[0].ranges).toBe("-3,4 +3,4")
    expect(hunks[0].lines.map(text)).toEqual(["# T", "", "The old body.", "The new body."])
  })

  it("numbers each line from the range it starts at, on the side that holds it", () => {
    const lines = parseDiff(modified)[0].lines
    expect(lines.map((line) => [line.kind, line.before, line.after])).toEqual([
      ["context", 3, 3],
      ["context", 4, 4],
      ["deleted", 5, null],
      ["added", null, 5],
    ])
  })

  it("reads every hunk of a diff", () => {
    const hunks = parseDiff(["@@ -1,2 +1,2 @@", "-one", "+ONE", "@@ -9,2 +9,2 @@", "-nine", "+NINE"].join("\n"))
    expect(hunks.map((hunk) => hunk.ranges)).toEqual(["-1,2 +1,2", "-9,2 +9,2"])
  })

  it("leaves out the marker git writes for a hunk with no trailing newline", () => {
    const hunks = parseDiff(
      ["@@ -1 +1 @@", "-one", "\\ No newline at end of file", "+two", "\\ No newline at end of file"].join("\n"),
    )
    expect(hunks[0].lines.map(text)).toEqual(["one", "two"])
  })

  it("marks the words that differ between a removed line and the added line that follows it", () => {
    const lines = parseDiff(modified)[0].lines
    expect(changed(lines[2])).toEqual(["old "])
    expect(changed(lines[3])).toEqual(["new "])
  })

  it("pairs a run of removed lines with the run of added lines by offset", () => {
    const lines = parseDiff(["@@ -1,2 +1,2 @@", "-one fish", "-two fish", "+one bird", "+two bird"].join("\n"))[0].lines
    expect(lines.map(changed)).toEqual([["fish"], ["fish"], ["bird"], ["bird"]])
  })

  it("leaves a rewrite with no word in common unmarked, because its colour already says so", () => {
    const lines = parseDiff(["@@ -1 +1 @@", "-alpha beta", "+gamma delta"].join("\n"))[0].lines
    expect(lines.map(changed)).toEqual([[], []])
  })

  it("leaves a removed line with no added partner unmarked", () => {
    const lines = parseDiff(["@@ -1,2 +1 @@", " kept", "-gone"].join("\n"))[0].lines
    expect(lines.map(changed)).toEqual([[], []])
  })

  it("finds no hunk in an empty diff", () => {
    expect(parseDiff("")).toEqual([])
  })
})
