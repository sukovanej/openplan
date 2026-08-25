import { beforeEach, describe, expect, it } from "vitest"

import { copyTargetRow, hoveredRow } from "../src/lib/copy-target"

beforeEach(() => {
  hoveredRow.clear()
})

const rendered = ["12", "13"]

describe("hovered row", () => {
  it("tracks the row the pointer is over", () => {
    expect(hoveredRow.among(rendered)).toBeUndefined()
    hoveredRow.enter("12", 0)
    expect(hoveredRow.among(rendered)).toBe("12")
    expect(hoveredRow.place(rendered)).toBe(0)
    hoveredRow.leave("12", 0)
    expect(hoveredRow.among(rendered)).toBeUndefined()
    expect(hoveredRow.place(rendered)).toBe(-1)
  })

  it("keeps the newer hover when the row left behind reports its leave last", () => {
    hoveredRow.enter("12", 0)
    hoveredRow.enter("13", 1)
    hoveredRow.leave("12", 0)
    expect(hoveredRow.among(rendered)).toBe("13")
  })

  it("drops a hover whose row is gone, since an unmounted row reports no leave", () => {
    hoveredRow.enter("13", 1)
    expect(hoveredRow.among(["12"])).toBeUndefined()
    expect(hoveredRow.among([])).toBeUndefined()
  })

  // A task detail can show one task under both "Blocks" and "Subtasks".
  it("tells two rows that name the same task apart by their place", () => {
    const twice = ["12", "13", "13"]
    hoveredRow.enter("13", 2)
    expect(hoveredRow.place(twice)).toBe(2)
    hoveredRow.leave("13", 1)
    expect(hoveredRow.place(twice)).toBe(2)
    hoveredRow.leave("13", 2)
    expect(hoveredRow.place(twice)).toBe(-1)
  })
})

describe("copyTargetRow", () => {
  it("prefers the hovered row over both the cursor and the route", () => {
    expect(copyTargetRow("12", "13", "14")).toBe("12")
  })

  it("falls back to the keyboard cursor when nothing is hovered", () => {
    expect(copyTargetRow(undefined, "13", "14")).toBe("13")
  })

  it("falls back to the route's own task when neither hover nor cursor is set", () => {
    expect(copyTargetRow(undefined, undefined, "14")).toBe("14")
  })

  it("resolves to nothing when there is no candidate at all", () => {
    expect(copyTargetRow(undefined, undefined, undefined)).toBeUndefined()
  })
})
