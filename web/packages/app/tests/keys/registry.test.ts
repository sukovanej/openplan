import { describe, expect, it } from "vitest"

import { bindings } from "../../src/lib/keys/bindings"
import { chordOf } from "../../src/lib/keys/match"
import { helpGroups } from "../../src/lib/keys/registry"

describe("help overlay content is derived from the registry", () => {
  const groups = helpGroups(bindings)

  it("renders one entry per non-overlay binding, in registry order", () => {
    const shown = bindings.filter((binding) => binding.scope !== "overlay")
    const flattened = groups.flatMap((group) => group.entries)
    expect(flattened.map((entry) => entry.id)).toEqual(shown.map((binding) => binding.id))
  })

  it("omits overlay-scope bindings, which are the modal's own dismiss keys", () => {
    const ids = groups.flatMap((group) => group.entries).map((entry) => entry.id)
    expect(ids).not.toContain("help.close.escape")
    expect(ids).not.toContain("help.close.question")
  })

  it("groups entries by their binding group", () => {
    expect(groups.map((group) => group.name)).toEqual(["Navigation", "Task", "Help"])
  })

  it("carries each binding's label and normalized keys verbatim", () => {
    const source = bindings.find((binding) => binding.id === "go.list")!
    const entry = groups.flatMap((group) => group.entries).find((item) => item.id === "go.list")!
    expect(entry.label).toBe(source.label)
    expect(entry.keys).toEqual(chordOf(source.keys))
    expect(entry.keys).toEqual(["g", "l"])
  })
})
