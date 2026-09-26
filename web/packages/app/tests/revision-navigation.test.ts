import { describe, expect, it } from "vitest"

import { isOverLiveTask, revisionNavigation } from "../src/lib/revision-navigation"

describe("revisionNavigation", () => {
  it("pushes the first revision over the live task, and marks it so", () => {
    const first = revisionNavigation(false, null)
    expect(first.replace).toBe(false)
    expect(isOverLiveTask(first.state)).toBe(true)
  })

  it("puts each next revision in the place of the one on screen, and keeps what that one knew", () => {
    const overLive = revisionNavigation(false, null).state
    expect(revisionNavigation(true, overLive)).toEqual({ replace: true, state: overLive })
    expect(isOverLiveTask(revisionNavigation(true, overLive).state)).toBe(true)
    expect(isOverLiveTask(revisionNavigation(true, null).state)).toBe(false)
  })
})

describe("isOverLiveTask", () => {
  it("holds for no state but the mark", () => {
    expect(isOverLiveTask(undefined)).toBe(false)
    expect(isOverLiveTask(null)).toBe(false)
    expect(isOverLiveTask({ overLiveTask: "yes" })).toBe(false)
  })
})
