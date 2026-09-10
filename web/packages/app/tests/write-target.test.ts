import { describe, expect, it } from "vitest"

import type { TaskListItem } from "@openplan/api-client"

import { writeHere } from "../src/lib/write-target"

const task = (write_target: TaskListItem["write_target"]): TaskListItem =>
  ({ id: "OPP-12", project: "openplan", write_target }) as TaskListItem

describe("where an edit lands", () => {
  it("names the branch the daemon resolved for the write", () => {
    expect(writeHere(task({ branch: "main", writable: true }))).toEqual({ branch: "main", blocked: undefined })
  })

  it("names the branch no worktree can write, and refuses the edit", () => {
    const { branch, blocked } = writeHere(task({ branch: "feature", writable: false }))
    expect(branch).toBe("feature")
    expect(blocked).toContain("feature")
  })

  it("refuses the edit when the repository offers no branch at all", () => {
    const { branch, blocked } = writeHere(task(undefined))
    expect(branch).toBeUndefined()
    expect(blocked).not.toBeUndefined()
  })
})
