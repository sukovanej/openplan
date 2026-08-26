import { expect, it } from "@effect/vitest"

import type { TaskDetail, TaskRef } from "@open-planner/api-client"

import { detailRows } from "../src/lib/detail-rows"

function ref(id: string, title: string): TaskRef {
  return { id, title, status: "todo" }
}

function detail(fields: Partial<TaskDetail>): TaskDetail {
  return {
    project: "plan",
    id: "OPP-2",
    title: "API",
    metadata: {
      status: "todo",
      created: "2026-01-01T00:00:00Z",
      parent: null,
      rank: null,
      dependencies: [],
      tags: [],
    },
    body: "",
    updated: { kind: "missing" },
    headline: "main",
    branches: [],
    ...fields,
  } as unknown as TaskDetail
}

const waiting = detail({
  metadata: {
    status: "todo",
    created: "2026-01-01T00:00:00Z",
    parent: null,
    rank: null,
    dependencies: ["OPP-3", "OPP-1#Wire", "OPP-1#Shape", "OPP-99"],
    tags: [],
  },
  depends_on: [ref("OPP-3", "Schema"), ref("OPP-1", "Design")],
  blocks: [ref("OPP-4", "Ship")],
  children: [ref("OPP-7", "Handler")],
})

it("keeps the order the file gives", () => {
  expect(detailRows("plan", waiting).dependsOn.map((row) => row.id)).toEqual(["OPP-3", "OPP-1", "OPP-1", "OPP-99"])
})

it("marks a dependency that names no task, which has no title and no status", () => {
  const gone = detailRows("plan", waiting).dependsOn[3]
  expect(gone).toEqual({
    at: 3,
    path: "/plan/task/OPP-99",
    id: "OPP-99",
    title: undefined,
    status: undefined,
    unresolved: true,
  })
})

it("names the task a sectioned dependency aims at, and links each entry to its own section", () => {
  const [, wire, shape] = detailRows("plan", waiting).dependsOn
  expect([wire.title, shape.title]).toEqual(["Design", "Design"])
  expect([wire.path, shape.path]).toEqual(["/plan/task/OPP-1#Wire", "/plan/task/OPP-1#Shape"])
})

it("numbers every row for the one cursor that walks the three lists in document order", () => {
  const rows = detailRows("plan", waiting)
  expect(rows.blocks.map((row) => row.at)).toEqual([4])
  expect(rows.subtasks.map((row) => row.at)).toEqual([5])
  expect(rows.paths).toEqual([
    "/plan/task/OPP-3",
    "/plan/task/OPP-1#Wire",
    "/plan/task/OPP-1#Shape",
    "/plan/task/OPP-99",
    "/plan/task/OPP-4",
    "/plan/task/OPP-7",
  ])
})

it("has no rows before the detail loads, so no section flashes unresolved", () => {
  expect(detailRows("plan", null)).toEqual({ dependsOn: [], blocks: [], subtasks: [], paths: [] })
})
