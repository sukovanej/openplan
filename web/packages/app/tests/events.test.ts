import { expect, it } from "@effect/vitest"
import { Schema } from "effect"

import { applyChange, ChangeEvent, coalesced, type Invalidator } from "../src/lib/events"

function spy() {
  const calls: {
    projects: number
    lists: Array<string>
    tasks: Array<string>
    history: Array<string>
    sync: Array<string>
    visible: Array<string | undefined>
  } = { projects: 0, lists: [], tasks: [], history: [], sync: [], visible: [] }
  const agentSessions = { reads: 0 }
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
    refreshAgentSessions: () => {
      agentSessions.reads += 1
    },
  }
  return { inv, calls, agentSessions }
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

function held() {
  const { inv, calls, agentSessions } = spy()
  const flushes: Array<() => void> = []
  const coalescing = coalesced(inv, (flush) => flushes.push(flush))
  const flush = () => {
    for (const one of flushes.splice(0)) one()
  }
  return { coalescing, calls, agentSessions, flush, flushes }
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

it("re-reads the one session list when a session changes in any project", () => {
  const { inv, calls, agentSessions } = spy()
  applyChange(inv, Schema.decodeUnknownSync(ChangeEvent)({ kind: "agent_sessions_changed", project: "openplan" }))
  expect(agentSessions.reads).toBe(1)
  expect(calls.lists).toEqual([])
  expect(calls.visible).toEqual([])
})

it("re-reads the session list once for a burst of session changes", () => {
  const { coalescing, agentSessions, flush } = held()
  applyChange(coalescing, { kind: "agent_sessions_changed", project: "openplan" })
  applyChange(coalescing, { kind: "agent_sessions_changed", project: "notes" })

  expect(agentSessions.reads).toBe(0)
  flush()
  expect(agentSessions.reads).toBe(1)
})
