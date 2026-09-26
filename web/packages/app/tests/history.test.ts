import { describe, expect, it } from "vitest"

import type { DocumentChange, HistoryEntry, TaskChange } from "@openplan/api-client"

import {
  activityRows,
  changePath,
  diffTarget,
  olderThan,
  otherChanges,
  SHOWN_CHANGES,
  taskChangeOf,
} from "../src/lib/history"

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

describe("the rows of a revision in the activity", () => {
  it("lists the tasks, then the tags, then the other documents", () => {
    const rows = activityRows({
      ...entry("r", [
        { path: "tags/bug.md", kind: "added", tag: "bug" },
        { path: "config.toml", kind: "added" },
      ]),
      tasks: [{ task: "OPP-1", kind: "added", title: "First" }],
      tags: [{ tag: "bug", kind: "added" }],
    })
    expect(rows.map((row) => row.kind)).toEqual(["task", "tag", "document"])
  })

  it("stops at a limit, and counts the rest", () => {
    const tasks: ReadonlyArray<TaskChange> = Array.from({ length: SHOWN_CHANGES + 3 }, (_, at) => ({
      task: `OPP-${at + 1}`,
      kind: "added",
    }))
    const rows = activityRows({ ...entry("r"), tasks })
    expect(rows).toHaveLength(SHOWN_CHANGES + 1)
    expect(rows.at(-1)).toEqual({ kind: "more", count: 3 })
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

describe("the document a change line diffs", () => {
  it("diffs a modified task in its one file", () => {
    const changed = entry("r", [{ path: "tasks/00001-one.md", kind: "modified", task: "OPP-1" }])
    expect(diffTarget(changed, { kind: "task", change: { task: "OPP-1", kind: "modified" } })).toEqual({
      path: "tasks/00001-one.md",
    })
  })

  it("diffs a removed task in the file it left", () => {
    const removed = entry("r", [{ path: "tasks/00001-one.md", kind: "removed", task: "OPP-1" }])
    expect(diffTarget(removed, { kind: "task", change: { task: "OPP-1", kind: "removed" } })).toEqual({
      path: "tasks/00001-one.md",
    })
  })

  it("diffs a moved task from its old file to its new one", () => {
    const moved = entry("r", [
      { path: "tasks/00001-one.md", kind: "removed", task: "OPP-1" },
      { path: "tasks/00001-uno.md", kind: "added", task: "OPP-1" },
    ])
    expect(diffTarget(moved, { kind: "task", change: { task: "OPP-1", kind: "modified" } })).toEqual({
      path: "tasks/00001-uno.md",
      from: "tasks/00001-one.md",
    })
  })

  it("diffs a renumbered task from the file of its old key", () => {
    const renumbered = entry("r", [
      { path: "tasks/00002-one.md", kind: "removed", task: "OPP-2" },
      { path: "tasks/00003-one.md", kind: "added", task: "OPP-3" },
    ])
    const change: TaskChange = {
      task: "OPP-3",
      kind: "modified",
      fields: [{ field: "number", from: "OPP-2", to: "OPP-3" }],
    }
    expect(diffTarget(renumbered, { kind: "task", change })).toEqual({
      path: "tasks/00003-one.md",
      from: "tasks/00002-one.md",
    })
  })

  it("diffs a renamed tag from its old file to its new one", () => {
    const renamed = entry("r", [
      { path: "tags/bug.toml", kind: "removed", tag: "bug" },
      { path: "tags/defect.toml", kind: "added", tag: "defect" },
    ])
    expect(
      diffTarget(renamed, { kind: "tag", change: { tag: "defect", kind: "modified", renamed_from: "bug" } }),
    ).toEqual({ path: "tags/defect.toml", from: "tags/bug.toml" })
  })

  it("diffs another document at its own path, and nothing for the rest", () => {
    const other = entry("r", [{ path: "config.toml", kind: "modified" }])
    expect(diffTarget(other, { kind: "document", change: { path: "config.toml", kind: "modified" } })).toEqual({
      path: "config.toml",
    })
    expect(diffTarget(other, { kind: "more", count: 3 })).toBeUndefined()
  })
})
