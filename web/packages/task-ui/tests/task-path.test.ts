import { describe, expect, it } from "vitest"

import { boardPath, taskPath, taskRouteOf } from "../src/task-path"

describe("boardPath", () => {
  it("spells the route for a project's board", () => {
    expect(boardPath("open-plan")).toBe("/open-plan")
  })
})

describe("taskPath", () => {
  it("spells the route for a task, with an optional encoded section", () => {
    expect(taskPath("open-plan", "OPP-42")).toBe("/open-plan/task/OPP-42")
    expect(taskPath("open-plan", "OPP-3", "Store DTOs")).toBe("/open-plan/task/OPP-3#Store%20DTOs")
  })
})

describe("taskRouteOf", () => {
  it("reads the project and the id back out of any spelling of the route", () => {
    expect(taskRouteOf("/web/task/28")).toEqual({ project: "web", id: "28" })
    expect(taskRouteOf("/web/task/28/")).toEqual({ project: "web", id: "28" })
    expect(taskRouteOf(taskPath("open-plan", "OPP-3", "Design"))).toEqual({ project: "open-plan", id: "OPP-3" })
  })

  // Two stores can commit the same abbreviation, so the same key on two projects is two tasks.
  it("keeps two projects' spellings of one key apart", () => {
    expect(taskRouteOf("/web/task/APP-1")?.project).toBe("web")
    expect(taskRouteOf("/api/task/APP-1")?.project).toBe("api")
  })

  it("has no task off the route, on a board route, or on an id-less path", () => {
    expect(taskRouteOf("/")).toBeUndefined()
    expect(taskRouteOf("/open-plan")).toBeUndefined()
    expect(taskRouteOf("/open-plan/task/")).toBeUndefined()
    expect(taskRouteOf("/task/OPP-1")).toBeUndefined()
  })
})
