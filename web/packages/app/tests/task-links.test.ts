import { expect, it } from "@effect/vitest"

import { splitTaskRefs } from "../src/components/task-links"

it("returns null when there is no reference", () => {
  expect(splitTaskRefs("plain text with no refs")).toBeNull()
})

it("links a bare task id", () => {
  expect(splitTaskRefs("see [[task-crud-6e8b]] first")).toEqual([
    { type: "text", value: "see " },
    { type: "link", url: "/task/task-crud-6e8b", title: null, children: [{ type: "text", value: "task-crud-6e8b" }] },
    { type: "text", value: " first" },
  ])
})

it("links every reference in a line", () => {
  const nodes = splitTaskRefs("[[a-1a1a]] and [[b-2b2b]]")
  expect(nodes?.map((n) => (n.type === "link" ? n.url : n.value))).toEqual([
    "/task/a-1a1a",
    " and ",
    "/task/b-2b2b",
  ])
})

it("keeps the section suffix in the label and encodes it in the hash", () => {
  const nodes = splitTaskRefs("[[task-crud-6e8b#Store DTOs]]")
  expect(nodes).toEqual([
    {
      type: "link",
      url: "/task/task-crud-6e8b#Store%20DTOs",
      title: null,
      children: [{ type: "text", value: "task-crud-6e8b#Store DTOs" }],
    },
  ])
})

it("leaves non-id bracket text untouched", () => {
  expect(splitTaskRefs("[[Some Page Title]]")).toBeNull()
  expect(splitTaskRefs("array[[index]]")).toBeNull()
})
