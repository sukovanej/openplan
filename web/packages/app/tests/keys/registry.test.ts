import { describe, expect, it } from "vitest"

import { bindings } from "../../src/lib/keys/bindings"
import { chordOf } from "../../src/lib/keys/match"
import { helpGroups } from "../../src/lib/keys/registry"
import { isOverlayScope } from "../../src/lib/keys/types"

describe("help overlay content is derived from the registry", () => {
  const groups = helpGroups(bindings)

  it("renders one entry per non-overlay binding, in registry order", () => {
    const shown = bindings.filter((binding) => !isOverlayScope(binding.scope) && binding.help !== "hidden")
    const flattened = groups.flatMap((group) => group.entries)
    expect(flattened.map((entry) => entry.id)).toEqual(shown.map((binding) => binding.id))
  })

  it("omits overlay-scope bindings, which are each overlay's own dismiss keys", () => {
    const ids = groups.flatMap((group) => group.entries).map((entry) => entry.id)
    expect(ids).not.toContain("help.close.escape")
    expect(ids).not.toContain("help.close.question")
    expect(ids).not.toContain("palette.close")
  })

  it("shows the digits that select a project as one line", () => {
    const entries = groups.flatMap((group) => group.entries)
    expect(entries.filter((entry) => entry.label === "Select one of the first nine projects")).toEqual([
      { id: "project.1", keys: ["1–9"], label: "Select one of the first nine projects" },
    ])
  })

  it("groups entries by their binding group", () => {
    expect(groups.map((group) => group.name)).toEqual(["Navigation", "Task", "Search", "Help"])
  })

  it("carries each binding's label and normalized keys verbatim", () => {
    const source = bindings.find((binding) => binding.id === "go.tasks")!
    const entry = groups.flatMap((group) => group.entries).find((item) => item.id === "go.tasks")!
    expect(entry.label).toBe(source.label)
    expect(entry.keys).toEqual(chordOf(source.keys))
    expect(entry.keys).toEqual(["g", "t"])
  })
})
