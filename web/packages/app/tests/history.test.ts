import { describe, expect, it } from "vitest"

import type { DocumentChange, HistoryEntry } from "@openplan/api-client"

import { changePath, olderThan, revisionChanges } from "../src/lib/history"

const entry = (id: string, changes: ReadonlyArray<DocumentChange> = [], parents = ["parent"]): HistoryEntry => ({
  revision: { id, parents, author: "Milan", at: "2026-01-02T00:00:00Z", message: `Revision ${id}` },
  changes,
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
  it("names each task once, apart from the documents that are not tasks", () => {
    const { tasks, others } = revisionChanges(
      entry("r", [
        { path: "tasks/00001-first.md", kind: "added", task: "OPP-1" },
        { path: "tags/backend.md", kind: "modified" },
        { path: "tasks/00002-second.md", kind: "removed", task: "OPP-2" },
      ]),
    )
    expect(tasks).toEqual([
      { id: "OPP-1", kind: "added" },
      { id: "OPP-2", kind: "removed" },
    ])
    expect(others.map((change) => change.path)).toEqual(["tags/backend.md"])
  })

  // A new title gives the file a new name, which the revision records as one removal and one addition.
  it("reads a task whose file moved as one modified task", () => {
    const { tasks } = revisionChanges(
      entry("r", [
        { path: "tasks/00001-old-title.md", kind: "removed", task: "OPP-1" },
        { path: "tasks/00001-new-title.md", kind: "added", task: "OPP-1" },
      ]),
    )
    expect(tasks).toEqual([{ id: "OPP-1", kind: "modified" }])
  })
})

describe("where a change leads", () => {
  it("opens a task that still exists as it is now", () => {
    expect(changePath("openplan", entry("r"), { id: "OPP-1", kind: "modified" }, true)).toBe("/openplan/task/OPP-1")
    expect(changePath("openplan", entry("r"), { id: "OPP-1", kind: "added" }, true)).toBe("/openplan/task/OPP-1")
  })

  it("opens a task that is gone as the revision left it", () => {
    expect(changePath("openplan", entry("r"), { id: "OPP-3", kind: "added" }, false)).toBe(
      "/openplan/task/OPP-3?revision=r",
    )
  })

  it("opens a removed task as it was just before the removal", () => {
    expect(changePath("openplan", entry("r", [], ["before"]), { id: "OPP-2", kind: "removed" }, false)).toBe(
      "/openplan/task/OPP-2?revision=before",
    )
  })

  it("has nowhere to open a task that a first revision removed", () => {
    expect(changePath("openplan", entry("r", [], []), { id: "OPP-2", kind: "removed" }, false)).toBeUndefined()
  })
})
