import { describe, expect, it } from "vitest"

import type { FieldChange } from "@openplan/api-client"

import { documentChangeText, fieldChangeText, renameText } from "../src/task-change"

describe("fieldChangeText", () => {
  it("says what a field was and what it is now", () => {
    expect(fieldChangeText({ field: "status", from: "in_progress", to: "in_review" })).toBe(
      "Status: In progress → In review",
    )
    expect(fieldChangeText({ field: "title", from: "Old", to: "New" })).toBe("Title: “Old” → “New”")
    expect(fieldChangeText({ field: "parent", from: "OPP-1", to: "OPP-2" })).toBe("Parent: OPP-1 → OPP-2")
    expect(fieldChangeText({ field: "parent", to: "OPP-2" })).toBe("Parent: OPP-2")
    expect(fieldChangeText({ field: "parent", from: "OPP-1" })).toBe("Parent removed")
    expect(fieldChangeText({ field: "number", from: "OPP-2", to: "OPP-3" })).toBe("Moved from OPP-2")
  })

  it("names what joined and what left a set", () => {
    expect(fieldChangeText({ field: "tags", from: ["bug", "draft"], to: ["bug", "feature"] })).toBe(
      "Tags: +feature −draft",
    )
    expect(fieldChangeText({ field: "dependencies", from: ["OPP-2", "OPP-1"], to: ["OPP-1", "OPP-2"] })).toBe(
      "Dependencies reordered",
    )
  })

  it("counts comments and conflicts", () => {
    const counted: ReadonlyArray<[FieldChange, string]> = [
      [{ field: "comments", added: 1, removed: 0 }, "1 comment"],
      [{ field: "comments", added: 2, removed: 0 }, "2 comments"],
      [{ field: "comments", added: 1, removed: 1 }, "Comments edited"],
      [{ field: "conflicts", from: 0, to: 2 }, "2 conflicts"],
      [{ field: "conflicts", from: 2, to: 0 }, "Conflicts resolved"],
    ]
    for (const [change, text] of counted) expect(fieldChangeText(change)).toBe(text)
  })

  it("names a field that openplan does not model", () => {
    expect(fieldChangeText({ field: "other", name: "estimate" })).toBe("Field estimate")
    expect(fieldChangeText({ field: "frontmatter" })).toBe("Unreadable frontmatter")
  })
})

describe("renameText", () => {
  it("says what happened to a tag or a doc, and the old name of a renamed one", () => {
    expect(renameText({ tag: "bug", kind: "added" })).toBe("Created")
    expect(renameText({ tag: "server", kind: "modified", renamed_from: "backend" })).toBe("Renamed from backend")
    expect(renameText({ doc: "storage", kind: "removed" })).toBe("Deleted")
    expect(renameText({ doc: "the-design", kind: "modified", renamed_from: "architecture" })).toBe(
      "Renamed from architecture",
    )
  })
})

describe("documentChangeText", () => {
  it("names each kind of change", () => {
    expect(documentChangeText("added")).toBe("Created")
    expect(documentChangeText("modified")).toBe("Edited")
    expect(documentChangeText("removed")).toBe("Deleted")
  })
})
