import { expect, it } from "@effect/vitest"
import { Schema } from "effect"

import { applyChange, ChangeEvent, type Invalidator } from "../src/lib/events"

function spy() {
  const calls: {
    projects: number
    lists: Array<string>
    tasks: Array<string>
    history: Array<string>
    sync: Array<string>
    visible: Array<string | undefined>
  } = { projects: 0, lists: [], tasks: [], history: [], sync: [], visible: [] }
  const inv: Invalidator = {
    refreshProjects: () => {
      calls.projects += 1
    },
    refreshList: (project) => {
      calls.lists.push(project)
    },
    refreshTask: (project, id) => {
      calls.tasks.push(`${project}/${id}`)
    },
    refreshHistory: (project) => {
      calls.history.push(project)
    },
    refreshSync: (project) => {
      calls.sync.push(project)
    },
    refreshVisible: (project) => {
      calls.visible.push(project)
    },
  }
  return { inv, calls }
}

const quiet = { projects: 0, lists: [], tasks: [], history: [], sync: [], visible: [] }
const decode = Schema.decodeUnknownSync(ChangeEvent)

it("decodes a task_changed event mirroring the Rust ChangeEvent JSON", () => {
  expect(decode({ kind: "task_changed", project: "openplan", id: "OPP-1" })).toEqual({
    kind: "task_changed",
    project: "openplan",
    id: "OPP-1",
  })
})

// A change in one project must leave every other project's reads alone, so each of these carries
// the project it happened in.
it("task_changed refreshes that task, that project's list, and that project's activity", () => {
  const { inv, calls } = spy()
  applyChange(inv, { kind: "task_changed", project: "openplan", id: "OPP-1" })
  expect(calls).toEqual({ ...quiet, lists: ["openplan"], tasks: ["openplan/OPP-1"], history: ["openplan"] })
})

it("decodes a tags_changed event mirroring the Rust ChangeEvent JSON", () => {
  expect(decode({ kind: "tags_changed", project: "openplan" })).toEqual({ kind: "tags_changed", project: "openplan" })
})

// A rename rewrites the tags of every task that references it, so the rows and the open detail can
// all read differently.
it("tags_changed refreshes that project's screen", () => {
  const { inv, calls } = spy()
  applyChange(inv, { kind: "tags_changed", project: "openplan" })
  expect(calls).toEqual({ ...quiet, visible: ["openplan"] })
})

it("decodes a sync_changed event mirroring the Rust ChangeEvent JSON", () => {
  expect(decode({ kind: "sync_changed", project: "openplan" })).toEqual({ kind: "sync_changed", project: "openplan" })
})

// The daemon syncs every 30 seconds and sends this each time. What a sync brings in arrives as task
// changes of its own, so this re-reads the sync state alone.
it("sync_changed re-reads only that project's sync state", () => {
  const { inv, calls } = spy()
  applyChange(inv, { kind: "sync_changed", project: "openplan" })
  expect(calls).toEqual({ ...quiet, sync: ["openplan"] })
})

// The event names no project, and an abbreviation spells every id on screen, so the project list
// and every read there is are re-read.
it("projects_changed re-reads the projects and everything on screen", () => {
  const { inv, calls } = spy()
  applyChange(inv, { kind: "projects_changed" })
  expect(calls).toEqual({ ...quiet, projects: 1, visible: [undefined] })
})

// The stream dropped events and cannot say which, so nothing on screen can be trusted.
it("resync re-reads the projects and everything on screen", () => {
  const { inv, calls } = spy()
  applyChange(inv, { kind: "resync" })
  expect(calls).toEqual({ ...quiet, projects: 1, visible: [undefined] })
})

it("daemon_stopping changes no read", () => {
  const { inv, calls } = spy()
  applyChange(inv, { kind: "daemon_stopping" })
  expect(calls).toEqual(quiet)
})

// The daemon no longer sends these, and one from an older daemon must not pass for a change.
it("refuses the events of the branch model", () => {
  expect(() => decode({ kind: "ref_moved", project: "openplan", branch: "main" })).toThrow()
  expect(() => decode({ kind: "rolling_updates_changed", project: "openplan" })).toThrow()
})
