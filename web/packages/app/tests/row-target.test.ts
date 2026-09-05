import { beforeEach, describe, expect, it } from "vitest"

import { taskPath } from "@openplan/task-ui"

import type { CursorState } from "../src/lib/row-cursor"
import { hoveredRow, taskAtHand } from "../src/lib/row-target"

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

describe("the task at hand", () => {
  const path = (id: string) => taskPath("openplan", id)
  const cursor = (index: number): CursorState => ({ rows: [path("12"), path("13")], index })

  it("prefers the hovered row over both the cursor and the route", () => {
    hoveredRow.enter(path("13"), 1)
    expect(taskAtHand(cursor(0), path("28"))).toEqual({ project: "openplan", id: "13" })
  })

  it("falls back to the keyboard cursor when nothing is hovered", () => {
    expect(taskAtHand(cursor(0), path("28"))).toEqual({ project: "openplan", id: "12" })
  })

  it("falls back to the route's own task when neither hover nor cursor is set", () => {
    expect(taskAtHand(cursor(-1), path("28"))).toEqual({ project: "openplan", id: "28" })
  })

  it("resolves to no task on a route that names none", () => {
    expect(taskAtHand(cursor(-1), "/flow")).toBeUndefined()
  })
})
