import { describe, expect, it } from "vitest"

import { activityPath, boardPath, revisionPath, taskPath, taskRouteOf } from "../src/task-path"

describe("boardPath", () => {
  it("spells the route for a project's board", () => {
    expect(boardPath("openplan")).toBe("/openplan")
  })
})

describe("taskPath", () => {
  it("spells the route for a task, with an optional encoded section", () => {
    expect(taskPath("openplan", "OPP-42")).toBe("/openplan/task/OPP-42")
    expect(taskPath("openplan", "OPP-3", "Store DTOs")).toBe("/openplan/task/OPP-3#Store%20DTOs")
  })
})

describe("activityPath", () => {
  it("spells the route for a project's activity", () => {
    expect(activityPath("openplan")).toBe("/openplan/activity")
  })
})

describe("revisionPath", () => {
  it("spells the route for a task as a revision left it, which is still that task's route", () => {
    expect(revisionPath("openplan", "OPP-42", "abc/1")).toBe("/openplan/task/OPP-42?revision=abc%2F1")
    expect(taskRouteOf(revisionPath("openplan", "OPP-42", "abc"))).toEqual({ project: "openplan", id: "OPP-42" })
  })
})

describe("taskRouteOf", () => {
  it("reads the project and the id back out of any spelling of the route", () => {
    expect(taskRouteOf("/web/task/28")).toEqual({ project: "web", id: "28" })
    expect(taskRouteOf("/web/task/28/")).toEqual({ project: "web", id: "28" })
    expect(taskRouteOf(taskPath("openplan", "OPP-3", "Design"))).toEqual({ project: "openplan", id: "OPP-3" })
  })

  // Two stores can commit the same abbreviation, so the same key on two projects is two tasks.
  it("keeps two projects' spellings of one key apart", () => {
    expect(taskRouteOf("/web/task/APP-1")?.project).toBe("web")
    expect(taskRouteOf("/api/task/APP-1")?.project).toBe("api")
  })

  it("has no task off the route, on a board route, or on an id-less path", () => {
    expect(taskRouteOf("/")).toBeUndefined()
    expect(taskRouteOf("/openplan")).toBeUndefined()
    expect(taskRouteOf("/openplan/task/")).toBeUndefined()
    expect(taskRouteOf("/task/OPP-1")).toBeUndefined()
  })
})
