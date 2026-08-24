import { expect, it } from "@effect/vitest"

import type { TaskListItem } from "@open-planner/api-client"

import { taskMatches } from "../src/lib/task-search"

function task(id: string, title: string): TaskListItem {
  return {
    id,
    title,
    metadata: {
      status: "todo",
      created: "2026-01-01T00:00:00Z",
      parent: null,
      rank: null,
      dependencies: [],
      tags: [],
    },
    updated: { kind: "missing" },
    headline: "main",
    branches: [],
  } as unknown as TaskListItem
}

const tasks = [task("OPP-1", "Ship login page"), task("OPP-2", "Open the parser"), task("OPP-21", "Presence")]

const titles = (query: string) => taskMatches(tasks, query, new Set()).map((m) => m.task.title)

it("finds a task by its whole key", () => {
  expect(titles("OPP-21")).toEqual(["Presence"])
})

it("narrows as the number is typed", () => {
  expect(titles("OPP-2")).toEqual(["Open the parser", "Presence"])
})

it("is case-insensitive about the prefix", () => {
  expect(titles("opp-1")).toEqual(["Ship login page"])
})

// Every id starts with the abbreviation, so a query that is only its letters must stay a title
// search: as a key match it would mark every task and rank titles out of the popover entirely —
// "Presence" would lead the list for "o" despite sharing not one letter with it.
it("keeps a query that merely prefixes the abbreviation a title search", () => {
  expect(titles("o")).toEqual(["Open the parser", "Ship login page"])
  expect(titles("op")).toEqual(["Open the parser", "Ship login page"])
  expect(titles("opp")).toEqual(["Open the parser"])
})

it("still fuzzy-matches titles", () => {
  expect(titles("ship")).toEqual(["Ship login page"])
  expect(titles("")).toEqual(["Open the parser", "Presence", "Ship login page"])
})

it("skips excluded tasks", () => {
  expect(taskMatches(tasks, "OPP-2", new Set(["OPP-21"])).map((m) => m.task.id)).toEqual(["OPP-2"])
})
