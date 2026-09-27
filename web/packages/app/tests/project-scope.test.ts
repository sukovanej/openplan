import { describe, expect, it } from "vitest"

import { pageOf, pagePath, selectedProjects, selects, switchProjectPath } from "../src/lib/project-scope"

describe("pagePath", () => {
  it("spells the pages of one project and of every project the same way", () => {
    expect(pagePath("tasks", undefined)).toBe("/")
    expect(pagePath("tasks", "conquer")).toBe("/conquer")
    expect(pagePath("docs", undefined)).toBe("/docs")
    expect(pagePath("docs", "conquer")).toBe("/conquer/docs")
    expect(pagePath("activity", undefined)).toBe("/activity")
    expect(pagePath("tags", "conquer")).toBe("/conquer/tags")
    expect(pagePath("flow", undefined)).toBe("/flow")
    expect(pagePath("flow", "conquer")).toBe("/flow?project=conquer")
  })
})

describe("pageOf", () => {
  it("puts a task under the tasks and a doc under the docs", () => {
    expect(pageOf("/")).toBe("tasks")
    expect(pageOf("/conquer/task/CON-1")).toBe("tasks")
    expect(pageOf("/conquer/doc/storage")).toBe("docs")
    expect(pageOf("/docs")).toBe("docs")
    expect(pageOf("/conquer/activity")).toBe("activity")
    expect(pageOf("/tags")).toBe("tags")
  })

  it("puts the flow under its own page", () => {
    expect(pageOf("/flow")).toBe("flow")
  })
})

describe("selectedProjects", () => {
  it("reads the project from the first segment of any page under it", () => {
    expect(selectedProjects("/conquer", "")).toEqual(["conquer"])
    expect(selectedProjects("/conquer/docs", "")).toEqual(["conquer"])
    expect(selectedProjects("/conquer/task/CON-1", "")).toEqual(["conquer"])
    expect(selectedProjects("/conquer/doc/storage", "")).toEqual(["conquer"])
    expect(selectedProjects("/open%20plan/tags", "")).toEqual(["open plan"])
  })

  it("selects every project on the pages above them all", () => {
    expect(selectedProjects("/", "")).toEqual([])
    expect(selectedProjects("/docs", "")).toEqual([])
    expect(selectedProjects("/flow", "")).toEqual([])
    expect(selectedProjects("/tags", "")).toEqual([])
    expect(selectedProjects("/activity", "")).toEqual([])
  })

  it("reads the flow's selection from its query", () => {
    expect(selectedProjects("/flow", "?project=a&project=b&status=todo")).toEqual(["a", "b"])
  })
})

describe("switchProjectPath", () => {
  it("keeps the list the reader is on", () => {
    expect(switchProjectPath("/", "", "conquer")).toBe("/conquer")
    expect(switchProjectPath("/conquer", "", undefined)).toBe("/")
    expect(switchProjectPath("/docs", "", "conquer")).toBe("/conquer/docs")
    expect(switchProjectPath("/conquer/docs", "", undefined)).toBe("/docs")
  })

  it("leaves a task or a doc for the list it sits in", () => {
    expect(switchProjectPath("/conquer/task/CON-1", "", "openplan")).toBe("/openplan")
    expect(switchProjectPath("/conquer/doc/storage", "", "openplan")).toBe("/openplan/docs")
    expect(switchProjectPath("/conquer/doc/storage", "", undefined)).toBe("/docs")
  })

  it("keeps the tags and the activity", () => {
    expect(switchProjectPath("/conquer/tags", "", "openplan")).toBe("/openplan/tags")
    expect(switchProjectPath("/conquer/activity", "", "openplan")).toBe("/openplan/activity")
    expect(switchProjectPath("/conquer/tags", "", undefined)).toBe("/tags")
    expect(switchProjectPath("/activity", "", "conquer")).toBe("/conquer/activity")
  })

  it("narrows the flow to the project, keeps its other filters, and drops the tasks it names", () => {
    expect(switchProjectPath("/flow", "?project=a&task=A-1&status=todo", "b")).toBe("/flow?project=b&status=todo")
    expect(switchProjectPath("/flow", "?project=a&task=A-1", undefined)).toBe("/flow")
  })
})

describe("selects", () => {
  it("tells whether a project is the whole selection, and undefined whether every project is", () => {
    expect(selects([], undefined)).toBe(true)
    expect(selects(["conquer"], "conquer")).toBe(true)
    expect(selects(["conquer"], undefined)).toBe(false)
    expect(selects(["conquer", "openplan"], "conquer")).toBe(false)
  })
})
