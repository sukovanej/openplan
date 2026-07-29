import { expect, it } from "@effect/vitest"

import { splitTaskRefs } from "../src/components/task-links"

it("returns null when there is no reference", () => {
  expect(splitTaskRefs("plain text with no refs")).toBeNull()
})

it("links a reference naming the target file", () => {
  expect(splitTaskRefs("see [[./00042-ship-login-page.md]] first")).toEqual([
    { type: "text", value: "see " },
    {
      type: "link",
      url: "/task/42",
      title: null,
      children: [{ type: "text", value: "./00042-ship-login-page.md" }],
    },
    { type: "text", value: " first" },
  ])
})

it("links a bare task id", () => {
  expect(splitTaskRefs("see [[42]] first")).toEqual([
    { type: "text", value: "see " },
    { type: "link", url: "/task/42", title: null, children: [{ type: "text", value: "42" }] },
    { type: "text", value: " first" },
  ])
})

it("links every reference in a line", () => {
  const nodes = splitTaskRefs("[[./00001-a.md]] and [[./00002-b.md]]")
  expect(nodes?.map((n) => (n.type === "link" ? n.url : n.value))).toEqual(["/task/1", " and ", "/task/2"])
})

it("keeps the section suffix in the label and encodes it in the hash", () => {
  const nodes = splitTaskRefs("[[./00003-store-dtos.md#Store DTOs]]")
  expect(nodes).toEqual([
    {
      type: "link",
      url: "/task/3#Store%20DTOs",
      title: null,
      children: [{ type: "text", value: "./00003-store-dtos.md#Store DTOs" }],
    },
  ])
})

it("reads the id out of the leading digits, whatever the slug says", () => {
  const nodes = splitTaskRefs("[[./00042-a-stale-title.md]]")
  expect(nodes?.[0].type === "link" && nodes[0].url).toBe("/task/42")
})

it("leaves text that names no task untouched", () => {
  expect(splitTaskRefs("[[Some Page Title]]")).toBeNull()
  expect(splitTaskRefs("array[[index]]")).toBeNull()
  expect(splitTaskRefs("[[task-crud-6e8b]]")).toBeNull()
  expect(splitTaskRefs("[[042]]")).toBeNull()
  expect(splitTaskRefs("[[./notes.md]]")).toBeNull()
  expect(splitTaskRefs("[[./00042-ship.txt]]")).toBeNull()
})
