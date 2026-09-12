import { describe, expect, it } from "vitest"

import {
  agentPath,
  boardPath,
  isAgentRoute,
  projectRouteOf,
  taskPath,
  taskRouteOf,
  taskSessionPath,
} from "../src/task-path"

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
  it("sits above the projects and carries the chosen one and the session in the query", () => {
    expect(agentPath()).toBe("/agent")
    expect(agentPath("openplan")).toBe("/agent?project=openplan")
    expect(agentPath("open plan", "0a1b")).toBe("/agent?project=open+plan&session=0a1b")
    expect(agentPath(undefined, "0a1b")).toBe("/agent?session=0a1b")
  })
})

describe("taskSessionPath", () => {
  it("is the task's page with the session on it", () => {
    expect(taskSessionPath("openplan", "OPP-42", "0a1b")).toBe("/openplan/task/OPP-42?session=0a1b")
  })
})

describe("isAgentRoute", () => {
  it("is the agent page with or without a query", () => {
    expect(isAgentRoute("/agent")).toBe(true)
    expect(isAgentRoute("/agent/")).toBe(true)
    expect(isAgentRoute("/agent?project=openplan&session=0a1b")).toBe(true)
  })

  it("is not a board, a task, a project named agent's board, or the root", () => {
    expect(isAgentRoute("/")).toBe(false)
    expect(isAgentRoute("/openplan")).toBe(false)
    expect(isAgentRoute("/openplan/agent")).toBe(false)
    expect(isAgentRoute("/openplan/task/OPP-1")).toBe(false)
  })
})

describe("projectRouteOf", () => {
  it("names the project of a board, a tags page, and a task", () => {
    expect(projectRouteOf("/openplan")).toBe("openplan")
    expect(projectRouteOf("/openplan/tags")).toBe("openplan")
    expect(projectRouteOf("/open%20plan/task/OPP-1")).toBe("open plan")
  })

  it("names none on the merged board, the flow, or the agent page", () => {
    expect(projectRouteOf("/")).toBeUndefined()
    expect(projectRouteOf("/flow")).toBeUndefined()
    expect(projectRouteOf("/flow?task=OPP-1")).toBeUndefined()
    expect(projectRouteOf("/agent?project=openplan")).toBeUndefined()
  })
})
