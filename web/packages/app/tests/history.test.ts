import { describe, expect, it } from "vitest"

import type { DocumentChange, HistoryEntry, TaskChange } from "@openplan/api-client"

import { changePath, olderThan, otherChanges, taskChangeOf } from "../src/lib/history"

const entry = (
  id: string,
  changes: ReadonlyArray<DocumentChange> = [],
  parents = ["parent"],
  tasks: ReadonlyArray<TaskChange> = [],
): HistoryEntry => ({
  revision: { id, parents, author: "Milan", at: "2026-01-02T00:00:00Z", message: `Revision ${id}` },
  changes,
  summary: [],
  tasks,
  tags: [],
})

describe("paging", () => {
  it("pages on from the oldest revision of a full page", () => {
    expect(olderThan([entry("c"), entry("b")], 2)).toBe("b")
  })

  it("stops after a page shorter than the limit", () => {
    expect(olderThan([entry("a")], 2)).toBeUndefined()
    expect(olderThan([], 2)).toBeUndefined()
  })
})

describe("what a revision changed", () => {
  it("keeps apart the documents that are neither a task nor a tag", () => {
    const others = otherChanges(
      entry("r", [
        { path: "tasks/00001-first.md", kind: "added", task: "OPP-1" },
        { path: "tags/backend.md", kind: "modified", tag: "backend" },
        { path: "config.toml", kind: "modified" },
      ]),
    )
    expect(others.map((change) => change.path)).toEqual(["config.toml"])
  })

  it("finds the change of one task", () => {
    const status: TaskChange = {
      task: "OPP-2",
      kind: "modified",
      fields: [{ field: "status", from: "todo", to: "done" }],
    }
    const found = entry("r", [], ["parent"], [{ task: "OPP-1", kind: "added" }, status])
    expect(taskChangeOf(found, "OPP-2")).toBe(status)
    expect(taskChangeOf(found, "OPP-3")).toBeUndefined()
  })
})

describe("where a change leads", () => {
  it("opens a task that still exists as it is now", () => {
    expect(changePath("openplan", entry("r"), { task: "OPP-1", kind: "modified" }, true)).toBe("/openplan/task/OPP-1")
    expect(changePath("openplan", entry("r"), { task: "OPP-1", kind: "added" }, true)).toBe("/openplan/task/OPP-1")
  })

  it("opens a task that is gone as the revision left it", () => {
    expect(changePath("openplan", entry("r"), { task: "OPP-3", kind: "added" }, false)).toBe(
      "/openplan/task/OPP-3?revision=r",
    )
  })

  it("opens a removed task as it was just before the removal", () => {
    expect(changePath("openplan", entry("r", [], ["before"]), { task: "OPP-2", kind: "removed" }, false)).toBe(
      "/openplan/task/OPP-2?revision=before",
    )
  })

  it("has nowhere to open a task that a first revision removed", () => {
    expect(changePath("openplan", entry("r", [], []), { task: "OPP-2", kind: "removed" }, false)).toBeUndefined()
  })
})
