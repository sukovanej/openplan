import { expect, it } from "@effect/vitest"
import { Schema } from "effect"

import { applyChange, ChangeEvent, type Invalidator } from "../src/lib/events"

function spy() {
  const calls: { config: number; list: number; tasks: Array<string>; visible: number } = {
    config: 0,
    list: 0,
    tasks: [],
    visible: 0,
  }
  const inv: Invalidator = {
    refreshConfig: () => {
      calls.config += 1
    },
    refreshList: () => {
      calls.list += 1
    },
    refreshTask: (id) => {
      calls.tasks.push(id)
    },
    refreshVisible: () => {
      calls.visible += 1
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

it("task_changed refreshes that task and the list", () => {
  const { inv, calls } = spy()
  applyChange(inv, { kind: "task_changed", project: "open-plan", id: "abc", branch: "" })
  expect(calls).toEqual({ config: 0, list: 1, tasks: ["abc"], visible: 0 })
})

it("ref_moved refreshes everything on screen, including the open detail", () => {
  const { inv, calls } = spy()
  applyChange(inv, { kind: "ref_moved", project: "open-plan", branch: "main" })
  expect(calls).toEqual({ config: 0, list: 0, tasks: [], visible: 1 })
})

it("presence_changed refreshes the list", () => {
  const { inv, calls } = spy()
  applyChange(inv, { kind: "presence_changed", project: "open-plan", task_id: "abc" })
  expect(calls).toEqual({ config: 0, list: 1, tasks: [], visible: 0 })
})

// The abbreviation spells every id on screen, so both the config and everything showing one are
// re-read.
it("projects_changed re-reads the config and everything on screen", () => {
  const { inv, calls } = spy()
  applyChange(inv, { kind: "projects_changed" })
  expect(calls).toEqual({ config: 1, list: 0, tasks: [], visible: 1 })
})

// The stream dropped events and cannot say which, so nothing on screen can be trusted.
it("resync re-reads the config and everything on screen", () => {
  const { inv, calls } = spy()
  applyChange(inv, { kind: "resync" })
  expect(calls).toEqual({ config: 1, list: 0, tasks: [], visible: 1 })
})
