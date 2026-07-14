import { expect, it } from "@effect/vitest"

import { clampIndex, emptyCursor, focused, focusedId, moved, rowCursor, withRows } from "../src/lib/row-cursor"

const rows = (n: number): Array<string> => Array.from({ length: n }, (_, i) => `t-${i}`)

it("clampIndex keeps an index within [0, count - 1], or -1 when empty", () => {
  expect(clampIndex(-3, 5)).toBe(0)
  expect(clampIndex(0, 5)).toBe(0)
  expect(clampIndex(4, 5)).toBe(4)
  expect(clampIndex(9, 5)).toBe(4)
  expect(clampIndex(2, 0)).toBe(-1)
})

it("the cursor cannot move past the last row or before the first", () => {
  const first = focused(withRows(emptyCursor, rows(3)), 0)
  expect(first.index).toBe(0)
  expect(moved(first, -1).index).toBe(0)

  const last = focused(first, 2)
  expect(last.index).toBe(2)
  expect(moved(last, 1).index).toBe(2)

  expect(focused(first, 99).index).toBe(2)
  expect(focused(first, -99).index).toBe(0)
})

it("the cursor clears when the task set changes", () => {
  const at2 = focused(withRows(emptyCursor, rows(5)), 2)
  expect(at2.index).toBe(2)

  const next = withRows(at2, ["a", "b", "c"])
  expect(next.ids).toEqual(["a", "b", "c"])
  expect(next.index).toBe(-1)
})

it("the cursor is preserved when the task set is unchanged", () => {
  const at2 = focused(withRows(emptyCursor, rows(5)), 2)
  expect(withRows(at2, rows(5))).toBe(at2)
})

it("an empty task set clears the cursor", () => {
  const empty = withRows(focused(withRows(emptyCursor, rows(5)), 2), [])
  expect(empty.index).toBe(-1)
  expect(focusedId(empty)).toBeUndefined()
})

it("focusedId reports the id under the cursor", () => {
  expect(focusedId(focused(withRows(emptyCursor, rows(3)), 1))).toBe("t-1")
})

it("the store notifies subscribers and drives the cursor", () => {
  let ticks = 0
  const unsubscribe = rowCursor.subscribe(() => {
    ticks += 1
  })

  rowCursor.setRows(["a", "b", "c"])
  expect(rowCursor.getSnapshot().index).toBe(-1)

  rowCursor.moveBy(1)
  expect(rowCursor.getSnapshot().index).toBe(0)

  rowCursor.moveBy(1)
  expect(rowCursor.getSnapshot().index).toBe(1)

  rowCursor.moveBy(-5)
  expect(rowCursor.getSnapshot().index).toBe(0)

  rowCursor.focus(2)
  expect(rowCursor.getSnapshot().index).toBe(2)

  rowCursor.setRows(["x", "y"])
  expect(rowCursor.getSnapshot().index).toBe(-1)

  expect(ticks).toBeGreaterThan(0)
  unsubscribe()
})
