import { describe, expect, it } from "vitest"

import { agentPath, agentRouteOf, boardPath, projectRouteOf, taskPath, taskRouteOf } from "../src/task-path"

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

describe("agentPath", () => {
  it("spells the page for a new task, for an existing one, and with the session it is attached to", () => {
    expect(agentPath("openplan")).toBe("/openplan/agent")
    expect(agentPath("openplan", "OPP-42")).toBe("/openplan/agent/OPP-42")
    expect(agentPath("openplan", "OPP-42", "0a1b")).toBe("/openplan/agent/OPP-42?session=0a1b")
    expect(agentPath("openplan", undefined, "0a1b")).toBe("/openplan/agent?session=0a1b")
  })
})

describe("agentRouteOf", () => {
  it("reads the project, and the task when the route names one", () => {
    expect(agentRouteOf("/openplan/agent")).toEqual({ project: "openplan", id: undefined })
    expect(agentRouteOf("/openplan/agent/")).toEqual({ project: "openplan", id: undefined })
    expect(agentRouteOf("/openplan/agent/OPP-42")).toEqual({ project: "openplan", id: "OPP-42" })
    expect(agentRouteOf("/openplan/agent/OPP-42?session=0a1b")).toEqual({ project: "openplan", id: "OPP-42" })
  })

  it("has no agent route on a board, a task, or the root", () => {
    expect(agentRouteOf("/")).toBeUndefined()
    expect(agentRouteOf("/openplan")).toBeUndefined()
    expect(agentRouteOf("/openplan/task/OPP-1")).toBeUndefined()
  })
})

describe("projectRouteOf", () => {
  it("names the project of a board, a tags page, a task, and an agent page", () => {
    expect(projectRouteOf("/openplan")).toBe("openplan")
    expect(projectRouteOf("/openplan/tags")).toBe("openplan")
    expect(projectRouteOf("/openplan/task/OPP-1")).toBe("openplan")
    expect(projectRouteOf("/open%20plan/agent")).toBe("open plan")
  })

  it("names none on the merged board or the flow", () => {
    expect(projectRouteOf("/")).toBeUndefined()
    expect(projectRouteOf("/flow")).toBeUndefined()
    expect(projectRouteOf("/flow?task=OPP-1")).toBeUndefined()
  })
})
