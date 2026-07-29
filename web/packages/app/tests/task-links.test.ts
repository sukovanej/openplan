import { expect, it } from "@effect/vitest"

import { splitTaskRefs } from "../src/components/task-links"

it("returns null when there is no reference", () => {
  expect(splitTaskRefs("plain text with no refs")).toBeNull()
})

it("links a bare task id", () => {
  expect(splitTaskRefs("see [[42]] first")).toEqual([
    { type: "text", value: "see " },
    { type: "link", url: "/task/42", title: null, children: [{ type: "text", value: "42" }] },
    { type: "text", value: " first" },
  ])
})

it("links every reference in a line", () => {
  const nodes = splitTaskRefs("[[1]] and [[2]]")
  expect(nodes?.map((n) => (n.type === "link" ? n.url : n.value))).toEqual(["/task/1", " and ", "/task/2"])
})

it("keeps the section suffix in the label and encodes it in the hash", () => {
  const nodes = splitTaskRefs("[[3#Store DTOs]]")
  expect(nodes).toEqual([
    {
      type: "link",
      url: "/task/3#Store%20DTOs",
      title: null,
      children: [{ type: "text", value: "3#Store DTOs" }],
    },
  ])
})

it("leaves non-id bracket text untouched", () => {
  expect(splitTaskRefs("[[Some Page Title]]")).toBeNull()
  expect(splitTaskRefs("array[[index]]")).toBeNull()
  expect(splitTaskRefs("[[task-crud-6e8b]]")).toBeNull()
  expect(splitTaskRefs("[[042]]")).toBeNull()
})
