import { describe, expect, it } from "vitest"

import { defaultProject } from "../src/lib/new-task"

describe("the project a new task goes to", () => {
  const writable = ["alpha", "beta", "gamma"]

  it("is the project on screen", () => {
    expect(defaultProject(writable, "gamma", "beta")).toBe("gamma")
  })

  it("is the project of the last task created when the screen shows every project", () => {
    expect(defaultProject(writable, undefined, "beta")).toBe("beta")
  })

  it("is the first project when nothing else names one", () => {
    expect(defaultProject(writable, undefined, undefined)).toBe("alpha")
  })

  it("passes over a project that cannot take a write", () => {
    expect(defaultProject(["alpha", "gamma"], "beta", "beta")).toBe("alpha")
  })

  it("is none when no project can take a write", () => {
    expect(defaultProject([], "beta", undefined)).toBeUndefined()
  })
})
