import { expect, it } from "vitest"

import { splitTaskRefs, taskLinkPlugins } from "../src/task-links"

const split = (value: string) => splitTaskRefs(value, { project: "openplan", abbreviation: "OPP" })

it("returns null when there is no reference", () => {
  expect(split("plain text with no refs")).toBeNull()
})

it("links a reference naming the target file", () => {
  expect(split("see [[./00042-ship-login-page.md]] first")).toEqual([
    { type: "text", value: "see " },
    {
      type: "link",
      url: "/openplan/task/OPP-42",
      title: null,
      children: [{ type: "text", value: "./00042-ship-login-page.md" }],
    },
    { type: "text", value: " first" },
  ])
})

it("links a key", () => {
  expect(split("see [[OPP-42]] first")).toEqual([
    { type: "text", value: "see " },
    { type: "link", url: "/openplan/task/OPP-42", title: null, children: [{ type: "text", value: "OPP-42" }] },
    { type: "text", value: " first" },
  ])
})

it("links every reference in a line", () => {
  const nodes = split("[[./00001-a.md]] and [[OPP-2]]")
  expect(nodes?.map((n) => (n.type === "link" ? n.url : n.value))).toEqual([
    "/openplan/task/OPP-1",
    " and ",
    "/openplan/task/OPP-2",
  ])
})

it("keeps the section suffix in the label and encodes it in the hash", () => {
  const nodes = split("[[./00003-store-dtos.md#Store DTOs]]")
  expect(nodes).toEqual([
    {
      type: "link",
      url: "/openplan/task/OPP-3#Store%20DTOs",
      title: null,
      children: [{ type: "text", value: "./00003-store-dtos.md#Store DTOs" }],
    },
  ])
})

it("keeps the section suffix on a key too", () => {
  const nodes = split("[[OPP-3#Design]]")
  expect(nodes?.[0].type === "link" && nodes[0].url).toBe("/openplan/task/OPP-3#Design")
})

it("reads the id out of the leading digits, whatever the slug says", () => {
  const nodes = split("[[./00042-a-stale-title.md]]")
  expect(nodes?.[0].type === "link" && nodes[0].url).toBe("/openplan/task/OPP-42")
})

it("leaves text that names no task untouched", () => {
  expect(split("[[Some Page Title]]")).toBeNull()
  expect(split("array[[index]]")).toBeNull()
  expect(split("[[task-crud-6e8b]]")).toBeNull()
  expect(split("[[./notes.md]]")).toBeNull()
  expect(split("[[./00042-ship.txt]]")).toBeNull()
})

// The key is the whole id above the store (§3.1), so the spellings it replaced name nothing — and
// neither does another store's key, which this store cannot resolve.
it("leaves a bare number, a padded key, and a foreign key as plain text", () => {
  expect(split("[[42]]")).toBeNull()
  expect(split("[[042]]")).toBeNull()
  expect(split("[[OPP-042]]")).toBeNull()
  expect(split("[[opp-42]]")).toBeNull()
  expect(split("[[WEB-7]]")).toBeNull()
  expect(split("[[OPPX-42]]")).toBeNull()
})

// Until the config arrives there is no way to tell this store's key from any other, so nothing is
// linked rather than something linked wrongly; the body re-renders once it lands.
it("links nothing while the abbreviation is unknown", () => {
  expect(
    splitTaskRefs("see [[OPP-42]] and [[./00001-a.md]]", { project: "openplan", abbreviation: undefined }),
  ).toBeNull()
})

// unified is handed `[attacher, options]` and calls the attacher to get the transformer; passing a
// transformer straight into `remarkPlugins` would have it invoked with no tree at all.
it("attaches as a remark plugin that rewrites the tree in place", () => {
  const [attacher, options] = taskLinkPlugins({ project: "openplan", abbreviation: "OPP" })
  const transform = attacher(options)
  const tree = {
    type: "root",
    children: [{ type: "paragraph", children: [{ type: "text", value: "see [[OPP-42]]" }] }],
  }
  transform(tree)
  expect(tree.children[0].children).toEqual([
    { type: "text", value: "see " },
    { type: "link", url: "/openplan/task/OPP-42", title: null, children: [{ type: "text", value: "OPP-42" }] },
  ])
})
