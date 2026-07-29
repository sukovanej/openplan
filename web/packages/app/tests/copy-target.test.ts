import { beforeEach, describe, expect, it } from "vitest"

import { copyTargetId, hoveredRow, routeTaskId } from "../src/lib/copy-target"

beforeEach(() => {
  hoveredRow.clear()
})

const rendered = ["12", "13"]

describe("hovered row", () => {
  it("tracks the row the pointer is over", () => {
    expect(hoveredRow.among(rendered)).toBeUndefined()
    hoveredRow.enter("12")
    expect(hoveredRow.among(rendered)).toBe("12")
    hoveredRow.leave("12")
    expect(hoveredRow.among(rendered)).toBeUndefined()
  })

  it("keeps the newer hover when the row left behind reports its leave last", () => {
    hoveredRow.enter("12")
    hoveredRow.enter("13")
    hoveredRow.leave("12")
    expect(hoveredRow.among(rendered)).toBe("13")
  })

  it("drops a hover whose row is gone, since an unmounted row reports no leave", () => {
    hoveredRow.enter("13")
    expect(hoveredRow.among(["12"])).toBeUndefined()
    expect(hoveredRow.among([])).toBeUndefined()
  })
})

describe("copyTargetId", () => {
  it("prefers the hovered row over both the cursor and the route", () => {
    expect(copyTargetId("12", "13", "14")).toBe("12")
  })

  it("falls back to the keyboard cursor when nothing is hovered", () => {
    expect(copyTargetId(undefined, "13", "14")).toBe("13")
  })

  it("falls back to the route's own task when neither hover nor cursor is set", () => {
    expect(copyTargetId(undefined, undefined, "14")).toBe("14")
  })

  it("resolves to nothing when there is no candidate at all", () => {
    expect(copyTargetId(undefined, undefined, undefined)).toBeUndefined()
  })
})

describe("routeTaskId", () => {
  it("reads the id of a detail route", () => {
    expect(routeTaskId("/task/28")).toBe("28")
    expect(routeTaskId("/task/28/")).toBe("28")
  })

  it("has no id on the list route or an id-less path", () => {
    expect(routeTaskId("/")).toBeUndefined()
    expect(routeTaskId("/task/")).toBeUndefined()
  })
})
