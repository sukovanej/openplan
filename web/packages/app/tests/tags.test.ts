import { describe, expect, it } from "vitest"

import type { TagView } from "@open-planner/api-client"

import { tagMatches, tagSpelled, tagsWith, tagsWithout } from "../src/lib/tags"

const view = (name: string, over: Partial<TagView> = {}): TagView => ({
  name,
  display: name,
  color: "blue",
  ...over,
})

const registry = (...names: ReadonlyArray<string>): ReadonlyMap<string, TagView> =>
  new Map(names.map((name) => [name, view(name)]))

describe("the set a tags write sends", () => {
  it("adds the picked name to the ones the task already carries", () => {
    expect(tagsWith(["backend"], registry("backend", "wip"), "wip")).toEqual(["backend", "wip"])
  })

  it("drops the removed name", () => {
    expect(tagsWithout(["backend", "wip"], registry("backend", "wip"), "wip")).toEqual(["backend"])
  })

  // Validation is strict whole-set, so a name this branch's registry does not hold refuses the whole
  // write — including one the task already carried. Every edit drops the danglings to land at all.
  it("prunes the names the registry does not hold", () => {
    const tags = registry("backend")
    expect(tagsWith(["backend", "infra"], tags, "backend")).toEqual(["backend"])
    expect(tagsWithout(["backend", "infra"], tags, "backend")).toEqual([])
  })

  it("takes a dangling name off without touching the registered ones", () => {
    expect(tagsWithout(["backend", "infra"], registry("backend"), "infra")).toEqual(["backend"])
  })

  it("never sends a name twice", () => {
    expect(tagsWith(["backend"], registry("backend"), "backend")).toEqual(["backend"])
  })
})

describe("the picker's options", () => {
  const all = [view("backend", { display: "Backend" }), view("build"), view("wip")]

  it("offers every unassigned tag when nothing is typed", () => {
    expect(tagMatches(all, "", new Set()).map(({ tag }) => tag.name)).toEqual(["backend", "build", "wip"])
  })

  it("leaves out the tags the task already carries", () => {
    expect(tagMatches(all, "", new Set(["build"])).map(({ tag }) => tag.name)).toEqual(["backend", "wip"])
  })

  it("ranks a tighter match first and marks what matched", () => {
    const [first, second] = tagMatches(all, "b", new Set())
    expect([first.tag.name, second.tag.name]).toEqual(["backend", "build"])
    expect(first.indices).toEqual([0])
  })
})

describe("the tag a typed name already spells", () => {
  const all = [view("front-end", { display: "Front End" })]

  // The picker offers to register a name only when the registry holds none, and it answers a name
  // the task already carries with "already on this task" rather than an empty list.
  it("finds the entry under either spelling, whatever the case", () => {
    expect(tagSpelled(all, "front-end")?.name).toBe("front-end")
    expect(tagSpelled(all, "Front End")?.name).toBe("front-end")
  })

  it("finds nothing for a name the registry does not hold", () => {
    expect(tagSpelled(all, "backend")).toBeUndefined()
  })
})
