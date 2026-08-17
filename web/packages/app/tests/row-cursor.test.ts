import { expect, it } from "@effect/vitest"

import {
  clampIndex,
  cleared,
  emptyCursor,
  focused,
  focusedRow,
  moved,
  rowCursor,
  subtaskCursor,
  withRows,
} from "../src/lib/row-cursor"

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

it("an unselected cursor starts from the row it is given", () => {
  const rowed = withRows(emptyCursor, rows(5))
  expect(moved(rowed, 1, "t-2").index).toBe(3)
  expect(moved(rowed, -1, "t-2").index).toBe(1)
  expect(moved(rowed, -1, "t-0").index).toBe(0)
  expect(moved(rowed, 1, "t-4").index).toBe(4)
})

it("an unselected cursor falls back to the first row", () => {
  const rowed = withRows(emptyCursor, rows(5))
  expect(moved(rowed, 1).index).toBe(0)
  expect(moved(rowed, -1).index).toBe(0)
  expect(moved(rowed, 1, "gone").index).toBe(0)
})

it("a selected cursor ignores the row it is given", () => {
  const at2 = focused(withRows(emptyCursor, rows(5)), 2)
  expect(moved(at2, 1, "t-4").index).toBe(3)
})

it("the store resumes from the hovered row", () => {
  rowCursor.setRows(["hovered-a", "hovered-b", "hovered-c"])
  rowCursor.moveBy(1, "hovered-b")
  expect(rowCursor.getSnapshot().index).toBe(2)
})

it("the cursor clears when the task set changes", () => {
  const at2 = focused(withRows(emptyCursor, rows(5)), 2)
  expect(at2.index).toBe(2)

  const next = withRows(at2, ["a", "b", "c"])
  expect(next.rows).toEqual(["a", "b", "c"])
  expect(next.index).toBe(-1)
})

it("the cursor is preserved when the task set is unchanged", () => {
  const at2 = focused(withRows(emptyCursor, rows(5)), 2)
  expect(withRows(at2, rows(5))).toBe(at2)
})

it("an empty task set clears the cursor", () => {
  const empty = withRows(focused(withRows(emptyCursor, rows(5)), 2), [])
  expect(empty.index).toBe(-1)
  expect(focusedRow(empty)).toBeUndefined()
})

it("clear sets the index to -1 and preserves the rows", () => {
  const at2 = focused(withRows(emptyCursor, rows(5)), 2)
  const empty = cleared(at2)
  expect(empty.index).toBe(-1)
  expect(empty.rows).toBe(at2.rows)
})

it("clear on an already-cleared cursor returns the same state", () => {
  const rowed = withRows(emptyCursor, rows(3))
  expect(cleared(rowed)).toBe(rowed)
})

it("focusedId reports the id under the cursor", () => {
  expect(focusedRow(focused(withRows(emptyCursor, rows(3)), 1))).toBe("t-1")
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

it("the subtask cursor starts a never-visited task unselected", () => {
  subtaskCursor.activate("fresh-task", ["a", "b", "c"])
  expect(subtaskCursor.getSnapshot().index).toBe(-1)
})

it("the subtask cursor remembers each task's focused row across activations", () => {
  subtaskCursor.activate("parent", ["p1", "p2", "p3"])
  subtaskCursor.moveBy(1)
  subtaskCursor.moveBy(1)
  expect(focusedRow(subtaskCursor.getSnapshot())).toBe("p2")

  subtaskCursor.activate("child", ["c1", "c2"])
  expect(subtaskCursor.getSnapshot().index).toBe(-1)

  subtaskCursor.activate("parent", ["p1", "p2", "p3"])
  expect(focusedRow(subtaskCursor.getSnapshot())).toBe("p2")
})

it("the subtask cursor clamps a remembered row when the task lost children", () => {
  subtaskCursor.activate("shrinking", ["a", "b", "c"])
  subtaskCursor.focus(2)
  subtaskCursor.activate("shrinking", ["a"])
  expect(subtaskCursor.getSnapshot().index).toBe(0)

  subtaskCursor.activate("shrinking", [])
  expect(subtaskCursor.getSnapshot().index).toBe(-1)
})

it("clear dismisses an active selection but does not churn when already cleared", () => {
  let ticks = 0
  const unsubscribe = rowCursor.subscribe(() => {
    ticks += 1
  })

  rowCursor.setRows(["a", "b", "c"])
  rowCursor.focus(1)
  expect(rowCursor.getSnapshot().index).toBe(1)

  ticks = 0
  rowCursor.clear()
  expect(rowCursor.getSnapshot().index).toBe(-1)
  expect(ticks).toBe(1)

  rowCursor.clear()
  expect(ticks).toBe(1)

  unsubscribe()
})
