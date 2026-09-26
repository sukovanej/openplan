import { expect, it } from "@effect/vitest"
import { Schema } from "effect"

import { applyChange, ChangeEvent, coalesced, type Invalidator } from "../src/lib/events"

function spy() {
  const calls: {
    projects: number
    lists: Array<string>
    tasks: Array<string>
    docs: Array<string>
    pages: Array<string>
    history: Array<string>
    sync: Array<string>
    visible: Array<string | undefined>
  } = { projects: 0, lists: [], tasks: [], docs: [], pages: [], history: [], sync: [], visible: [] }
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
    refreshDoc: (project, name) => {
      calls.docs.push(`${project}/${name}`)
    },
    refreshPages: (project) => {
      calls.pages.push(project)
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

const quiet = { projects: 0, lists: [], tasks: [], docs: [], pages: [], history: [], sync: [], visible: [] }
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
it("task_changed refreshes that task, that project's pages, list, and activity", () => {
  const { inv, calls } = spy()
  applyChange(inv, { kind: "task_changed", project: "openplan", id: "OPP-1" })
  expect(calls).toEqual({
    ...quiet,
    lists: ["openplan"],
    tasks: ["openplan/OPP-1"],
    pages: ["openplan"],
    history: ["openplan"],
  })
})

it("decodes a doc_changed event mirroring the Rust ChangeEvent JSON", () => {
  expect(decode({ kind: "doc_changed", project: "openplan", name: "architecture" })).toEqual({
    kind: "doc_changed",
    project: "openplan",
    name: "architecture",
  })
})

it("doc_changed refreshes that doc, that project's pages and activity, and no task list", () => {
  const { inv, calls } = spy()
  applyChange(inv, { kind: "doc_changed", project: "openplan", name: "architecture" })
  expect(calls).toEqual({ ...quiet, docs: ["openplan/architecture"], pages: ["openplan"], history: ["openplan"] })
})

it("refreshes a doc once for a burst of changes to it", () => {
  const { coalescing, calls, flush } = held()
  for (const name of ["architecture", "storage", "architecture"]) {
    applyChange(coalescing, { kind: "doc_changed", project: "openplan", name })
  }
  applyChange(coalescing, { kind: "doc_changed", project: "notes", name: "architecture" })
  flush()
  expect(calls.docs).toEqual(["openplan/architecture", "openplan/storage", "notes/architecture"])
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
  applyChange(inv, { kind: "daemon_stopping", reason: "stop" })
  expect(calls).toEqual(quiet)
})

// The daemon no longer sends these, and one from an older daemon must not pass for a change.
it("refuses the events of the branch model", () => {
  expect(() => decode({ kind: "ref_moved", project: "openplan", branch: "main" })).toThrow()
  expect(() => decode({ kind: "rolling_updates_changed", project: "openplan" })).toThrow()
})

function held() {
  const { inv, calls } = spy()
  const flushes: Array<() => void> = []
  const coalescing = coalesced(inv, (flush) => flushes.push(flush))
  const flush = () => {
    for (const one of flushes.splice(0)) one()
  }
  return { coalescing, calls, flush, flushes }
}

// A sync that brings in many tasks sends one task_changed for each of them.
it("refreshes a list once for a burst of task changes, and each changed task once", () => {
  const { coalescing, calls, flush, flushes } = held()
  for (const id of ["OPP-1", "OPP-2", "OPP-1"]) {
    applyChange(coalescing, { kind: "task_changed", project: "openplan", id })
  }
  applyChange(coalescing, { kind: "task_changed", project: "notes", id: "NTS-1" })

  expect(calls).toEqual(quiet)
  expect(flushes).toHaveLength(1)
  flush()
  expect(calls).toEqual({
    ...quiet,
    lists: ["openplan", "notes"],
    tasks: ["openplan/OPP-1", "openplan/OPP-2", "notes/NTS-1"],
    pages: ["openplan", "notes"],
    history: ["openplan", "notes"],
  })
})

it("holds the refreshes that come after a flush for the next flush", () => {
  const { coalescing, calls, flush } = held()
  applyChange(coalescing, { kind: "sync_changed", project: "openplan" })
  flush()
  applyChange(coalescing, { kind: "sync_changed", project: "openplan" })
  expect(calls.sync).toEqual(["openplan"])
  flush()
  expect(calls.sync).toEqual(["openplan", "openplan"])
})

// A tag rename sends a task_changed for each task that carries the tag, and a tags_changed after them.
it("lets a refresh of a project's screen cover the narrower refreshes in that project", () => {
  const { coalescing, calls, flush } = held()
  applyChange(coalescing, { kind: "task_changed", project: "openplan", id: "OPP-1" })
  applyChange(coalescing, { kind: "sync_changed", project: "openplan" })
  applyChange(coalescing, { kind: "tags_changed", project: "openplan" })
  applyChange(coalescing, { kind: "task_changed", project: "notes", id: "NTS-1" })
  flush()
  expect(calls).toEqual({
    ...quiet,
    visible: ["openplan"],
    lists: ["notes"],
    tasks: ["notes/NTS-1"],
    pages: ["notes"],
    history: ["notes"],
  })
})

it("lets a refresh of every screen cover every other refresh but the projects", () => {
  const { coalescing, calls, flush } = held()
  applyChange(coalescing, { kind: "task_changed", project: "openplan", id: "OPP-1" })
  applyChange(coalescing, { kind: "tags_changed", project: "notes" })
  applyChange(coalescing, { kind: "resync" })
  flush()
  expect(calls).toEqual({ ...quiet, projects: 1, visible: [undefined] })
})

// A page the flush reads again by name needs no second read from the refresh of every page.
it("names to the pages every task and doc the flush refreshes in that project", () => {
  const { inv } = spy()
  const refreshed: Array<{ project: string; tasks: Array<string>; docs: Array<string> }> = []
  const target: Invalidator = {
    ...inv,
    refreshPages: (project, pages) => {
      refreshed.push({ project, tasks: [...pages.tasks], docs: [...pages.docs] })
    },
  }
  const flushes: Array<() => void> = []
  const coalescing = coalesced(target, (flush) => flushes.push(flush))
  applyChange(coalescing, { kind: "task_changed", project: "openplan", id: "OPP-1" })
  applyChange(coalescing, { kind: "doc_changed", project: "openplan", name: "architecture" })
  applyChange(coalescing, { kind: "task_changed", project: "notes", id: "NTS-1" })
  for (const flush of flushes.splice(0)) flush()

  expect(refreshed).toEqual([
    { project: "openplan", tasks: ["OPP-1"], docs: ["architecture"] },
    { project: "notes", tasks: ["NTS-1"], docs: [] },
  ])
})
