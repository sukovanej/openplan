import { expect, it } from "@effect/vitest"
import { Schema } from "effect"

import { applyChange, ChangeEvent, type Invalidator } from "../src/lib/events"

function spy() {
  const calls: {
    projects: number
    lists: Array<string>
    tasks: Array<string>
    visible: Array<string | undefined>
  } = { projects: 0, lists: [], tasks: [], visible: [] }
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
    refreshVisible: (project) => {
      calls.visible.push(project)
    },
  }
  return { inv, calls }
}

it("decodes a task_changed event mirroring the Rust ChangeEvent JSON", () => {
  const decoded = Schema.decodeUnknownSync(ChangeEvent)({
    kind: "task_changed",
    project: "open-plan",
    id: "abc",
    branch: "main",
  })
  expect(decoded).toEqual({
    kind: "task_changed",
    project: "open-plan",
    id: "abc",
    branch: "main",
  })
})

// A change in one project must leave every other project's reads alone, so each of these carries
// the project it happened in.
it("task_changed refreshes that task and that project's list", () => {
  const { inv, calls } = spy()
  applyChange(inv, { kind: "task_changed", project: "open-plan", id: "abc", branch: "" })
  expect(calls).toEqual({ projects: 0, lists: ["open-plan"], tasks: ["open-plan/abc"], visible: [] })
})

it("ref_moved refreshes that project's screen, including the open detail", () => {
  const { inv, calls } = spy()
  applyChange(inv, { kind: "ref_moved", project: "open-plan", branch: "main" })
  expect(calls).toEqual({ projects: 0, lists: [], tasks: [], visible: ["open-plan"] })
})

it("decodes a tags_changed event mirroring the Rust ChangeEvent JSON", () => {
  const decoded = Schema.decodeUnknownSync(ChangeEvent)({
    kind: "tags_changed",
    project: "open-plan",
    branch: "main",
  })
  expect(decoded).toEqual({ kind: "tags_changed", project: "open-plan", branch: "main" })
})

// A rename rewrites the tags of every task that references it, so the rows and the open detail can
// all read differently.
it("tags_changed refreshes that project's screen", () => {
  const { inv, calls } = spy()
  applyChange(inv, { kind: "tags_changed", project: "open-plan", branch: "main" })
  expect(calls).toEqual({ projects: 0, lists: [], tasks: [], visible: ["open-plan"] })
})

it("presence_changed refreshes that project's list", () => {
  const { inv, calls } = spy()
  applyChange(inv, { kind: "presence_changed", project: "open-plan", task_id: "abc" })
  expect(calls).toEqual({ projects: 0, lists: ["open-plan"], tasks: [], visible: [] })
})

// The event names no project, and an abbreviation spells every id on screen, so the project list
// and every read there is are re-read.
it("projects_changed re-reads the projects and everything on screen", () => {
  const { inv, calls } = spy()
  applyChange(inv, { kind: "projects_changed" })
  expect(calls).toEqual({ projects: 1, lists: [], tasks: [], visible: [undefined] })
})

// The stream dropped events and cannot say which, so nothing on screen can be trusted.
it("resync re-reads the projects and everything on screen", () => {
  const { inv, calls } = spy()
  applyChange(inv, { kind: "resync" })
  expect(calls).toEqual({ projects: 1, lists: [], tasks: [], visible: [undefined] })
})
