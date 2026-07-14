import { expect, it } from "@effect/vitest"
import { Schema } from "effect"

import { applyChange, ChangeEvent, type Invalidator } from "../src/lib/events"

function spy() {
  const calls: { list: number; tasks: Array<string> } = { list: 0, tasks: [] }
  const inv: Invalidator = {
    refreshList: () => {
      calls.list += 1
    },
    refreshTask: (id) => {
      calls.tasks.push(id)
    },
  }
  return { inv, calls }
}

it("decodes a task_changed event mirroring the Rust ChangeEvent JSON", () => {
  const decoded = Schema.decodeUnknownSync(ChangeEvent)({
    kind: "task_changed",
    id: "abc",
    branch: "main",
  })
  expect(decoded).toEqual({ kind: "task_changed", id: "abc", branch: "main" })
})

it("task_changed refreshes that task and the list", () => {
  const { inv, calls } = spy()
  applyChange(inv, { kind: "task_changed", id: "abc", branch: "" })
  expect(calls).toEqual({ list: 1, tasks: ["abc"] })
})

it("coarse events refresh only the list", () => {
  const { inv, calls } = spy()
  applyChange(inv, { kind: "ref_moved", branch: "main" })
  applyChange(inv, { kind: "presence_changed", task_id: "abc" })
  expect(calls).toEqual({ list: 2, tasks: [] })
})
