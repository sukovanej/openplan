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
    dependencies: ["OPP-3", "OPP-1#Wire", "OPP-99"],
    tags: [],
  },
  depends_on: [ref("OPP-3", "Schema"), ref("OPP-1", "Design")],
  blocks: [ref("OPP-4", "Ship")],
  children: [ref("OPP-7", "Handler")],
})

it("keeps the order the file gives", () => {
  expect(detailRows("plan", waiting).dependsOn.map((row) => row.id)).toEqual(["OPP-3", "OPP-1", "OPP-99"])
})

it("marks a dependency that names no task, which has no title and no status", () => {
  const [, , gone] = detailRows("plan", waiting).dependsOn
  expect(gone).toEqual({
    path: "/plan/task/OPP-99",
    id: "OPP-99",
    title: undefined,
    status: undefined,
    unresolved: true,
  })
})

it("names the task a sectioned dependency aims at, and links to the section", () => {
  const [, sectioned] = detailRows("plan", waiting).dependsOn
  expect(sectioned.id).toBe("OPP-1")
  expect(sectioned.title).toBe("Design")
  expect(sectioned.path).toBe("/plan/task/OPP-1#Wire")
})

it("sequences the three lists in document order for the one cursor that walks them", () => {
  expect(detailRows("plan", waiting).paths).toEqual([
    "/plan/task/OPP-3",
    "/plan/task/OPP-1#Wire",
    "/plan/task/OPP-99",
    "/plan/task/OPP-4",
    "/plan/task/OPP-7",
  ])
})

it("has no rows before the detail loads, so no section flashes unresolved", () => {
  expect(detailRows("plan", null)).toEqual({ dependsOn: [], blocks: [], subtasks: [], paths: [] })
})
