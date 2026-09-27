import { describe, expect, it } from "vitest"

import {
  activityPath,
  boardPath,
  docPath,
  docsPath,
  isActivityPath,
  isDocsPath,
  isTagsPath,
  projectOfPath,
  revisionPath,
  tagsPath,
  taskPath,
  taskRouteOf,
} from "../src/task-path"

describe("boardPath", () => {
  it("spells the route for a project's board, and for the board of every project", () => {
    expect(boardPath("openplan")).toBe("/openplan")
    expect(boardPath(undefined)).toBe("/")
  })
})

describe("docsPath", () => {
  it("spells the docs of a project under the project, and the docs of every project above them", () => {
    expect(docsPath("open plan")).toBe("/open%20plan/docs")
    expect(docsPath(undefined)).toBe("/docs")
  })
})

describe("projectOfPath", () => {
  it("reads the project from the first segment", () => {
    expect(projectOfPath("/openplan")).toBe("openplan")
    expect(projectOfPath(docsPath("open plan"))).toBe("open plan")
    expect(projectOfPath(taskPath("openplan", "OPP-1"))).toBe("openplan")
  })

  it("has no project on the pages above every project", () => {
    expect(projectOfPath("/")).toBeUndefined()
    expect(projectOfPath("/docs")).toBeUndefined()
    expect(projectOfPath("/flow")).toBeUndefined()
    expect(projectOfPath("/tags")).toBeUndefined()
    expect(projectOfPath("/activity")).toBeUndefined()
  })
})

describe("isDocsPath", () => {
  it("covers both docs lists and a doc's page", () => {
    expect(isDocsPath("/docs")).toBe(true)
    expect(isDocsPath(docsPath("openplan"))).toBe(true)
    expect(isDocsPath(docPath("openplan", "storage"))).toBe(true)
  })

  it("leaves out the task pages", () => {
    expect(isDocsPath("/")).toBe(false)
    expect(isDocsPath("/openplan")).toBe(false)
    expect(isDocsPath(taskPath("openplan", "OPP-1"))).toBe(false)
  })
})

describe("tagsPath", () => {
  it("spells the tags of a project under the project, and the tags of every project above them", () => {
    expect(tagsPath("open plan")).toBe("/open%20plan/tags")
    expect(tagsPath(undefined)).toBe("/tags")
  })
})

describe("isTagsPath", () => {
  it("covers the tags of a project and of every project", () => {
    expect(isTagsPath(tagsPath("openplan"))).toBe(true)
    expect(isTagsPath("/tags")).toBe(true)
    expect(isTagsPath("/openplan")).toBe(false)
    expect(isTagsPath("/openplan/activity")).toBe(false)
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
    expect(activityPath(undefined)).toBe("/activity")
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

describe("isActivityPath", () => {
  it("covers the activity of a project and of every project", () => {
    expect(isActivityPath(activityPath("open plan"))).toBe(true)
    expect(isActivityPath("/web/activity/")).toBe(true)
    expect(isActivityPath("/activity")).toBe(true)
  })

  it("leaves out every other page", () => {
    expect(isActivityPath("/")).toBe(false)
    expect(isActivityPath("/web")).toBe(false)
    expect(isActivityPath("/web/tags")).toBe(false)
    expect(isActivityPath("/web/activity/more")).toBe(false)
  })
})
