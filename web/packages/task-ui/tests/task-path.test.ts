import { describe, expect, it } from "vitest"

import { taskIdOf, taskPath } from "../src/task-path"

describe("taskPath", () => {
  it("spells the route for a task, with an optional encoded section", () => {
    expect(taskPath("OPP-42")).toBe("/task/OPP-42")
    expect(taskPath("OPP-3", "Store DTOs")).toBe("/task/OPP-3#Store%20DTOs")
  })
})

describe("taskIdOf", () => {
  it("reads the id back out of any spelling of the route", () => {
    expect(taskIdOf("/task/28")).toBe("28")
    expect(taskIdOf("/task/28/")).toBe("28")
    expect(taskIdOf(taskPath("OPP-3", "Design"))).toBe("OPP-3")
  })

  it("has no id off the route or on an id-less path", () => {
    expect(taskIdOf("/")).toBeUndefined()
    expect(taskIdOf("/task/")).toBeUndefined()
  })
})
