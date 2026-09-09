import { expect, it } from "@effect/vitest"

import type { MatrixCell } from "@openplan/api-client"

import { conflicted, pendingCount, type ProjectUpdates, syncState } from "../src/lib/rolling-updates"

const cell = (id: string): MatrixCell => ({
  branch: "openplan/rolling-updates",
  task: {
    id,
    title: id,
    metadata: { status: "todo", created: "2026-01-01T00:00:00Z", parent: null, rank: null, dependencies: [], tags: [] },
  },
  blob_oid: id,
  dirty: false,
  kind: "modified",
})

const updates = (project: string, pending: number, conflict = false): ProjectUpdates => ({
  project,
  pending: Array.from({ length: pending }, (_, at) => cell(`${project}-${at}`)),
  conflict: conflict ? { files: [".plan/tasks/00001-t.md"], worktree: "/tmp/updates" } : undefined,
})

it("counts what every project has waiting", () => {
  expect(pendingCount([updates("a", 2), updates("b", 3)])).toBe(5)
})

it("names the projects a conflict holds", () => {
  expect(conflicted([updates("a", 2), updates("b", 1, true)]).map((one) => one.project)).toEqual(["b"])
})

it("is idle when no project has anything waiting", () => {
  expect(syncState([updates("a", 0)], true, false)).toBe("idle")
})

it("is pending when a project has something waiting", () => {
  expect(syncState([updates("a", 0), updates("b", 1)], true, false)).toBe("pending")
})

// A conflict is what stops the count from ever being published, so it outranks the count.
it("is blocked when a conflict holds a project, whatever else is waiting", () => {
  expect(syncState([updates("a", 4), updates("b", 1, true)], true, false)).toBe("blocked")
})

it("is syncing while a publish runs", () => {
  expect(syncState([updates("a", 1, true)], true, true)).toBe("syncing")
})

// Nothing can publish while the daemon is down, and what the control holds is already stale.
it("is offline whatever the daemon last reported", () => {
  expect(syncState([updates("a", 3, true)], false, true)).toBe("offline")
})
