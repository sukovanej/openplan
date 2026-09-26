import { describe, expect, it } from "vitest"

import { DiffView } from "../src/diff-view"
import { render } from "./render"

const diff = [
  "--- a/.plan/tasks/00001-t.md",
  "+++ b/.plan/tasks/00001-t.md",
  "@@ -3,3 +3,3 @@",
  " # T",
  "-The old body.",
  "+The new body.",
  "",
].join("\n")

// Each line is three cells of one grid — two gutters and the text — and the hunk separator is a
// `div`, so the spans alone read as the lines in order.
const cells = (container: HTMLElement) => Array.from(container.querySelectorAll<HTMLElement>(".grid > span"))
const rows = (container: HTMLElement) => cells(container).filter((_, at) => at % 3 === 2)

describe("DiffView", () => {
  it("gives every line a number on the side that holds it", () => {
    const gutters = cells(render(<DiffView diff={diff} />)).map((cell) => cell.textContent)
    expect(gutters.slice(0, 2)).toEqual(["3", "3"])
    expect(gutters.slice(3, 5)).toEqual(["4", ""])
    expect(gutters.slice(6, 8)).toEqual(["", "4"])
  })

  it("signs each line by kind and colours the two that changed", () => {
    const container = render(<DiffView diff={diff} />)
    expect(rows(container).map((row) => row.textContent)).toEqual([" # T", "-The old body.", "+The new body."])
    expect(rows(container)[1].className).toContain("text-change-deleted")
    expect(rows(container)[2].className).toContain("text-change-added")
  })

  it("marks the words that changed inside the pair", () => {
    const container = render(<DiffView diff={diff} />)
    expect(Array.from(container.querySelectorAll("mark")).map((word) => word.textContent)).toEqual(["old ", "new "])
  })

  it("separates the hunks with their ranges", () => {
    const container = render(<DiffView diff={["@@ -1,1 +1,1 @@", "-one", "@@ -9,1 +9,1 @@", "+nine"].join("\n")} />)
    expect(Array.from(container.querySelectorAll(".col-span-3")).map((row) => row.textContent)).toEqual([
      "@@ -1,1 +1,1 @@",
      "@@ -9,1 +9,1 @@",
    ])
  })

  it("says so when the two sides agree", () => {
    expect(render(<DiffView diff="" />).textContent).toBe("No differences")
  })

  it("drops the file lines, which the caller renders itself", () => {
    expect(render(<DiffView diff={diff} />).textContent).not.toContain("00001-t.md")
  })
})
