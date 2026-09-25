import { describe, expect, it } from "vitest"

import type { FieldChange } from "@openplan/api-client"

import { fieldChangeText, tagChangeText, taskChangeText } from "../src/task-change"

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

describe("taskChangeText", () => {
  it("joins the fields of an edit", () => {
    expect(
      taskChangeText({
        task: "OPP-1",
        kind: "modified",
        title: "Ship it",
        fields: [{ field: "status", from: "todo", to: "done" }, { field: "description" }],
      }),
    ).toBe("Status: Todo → Done · Description")
  })

  it("says that a task came or went", () => {
    expect(taskChangeText({ task: "OPP-1", kind: "added", title: "Ship it" })).toBe("Created")
    expect(taskChangeText({ task: "OPP-1", kind: "removed", title: "Ship it" })).toBe("Deleted “Ship it”")
    expect(taskChangeText({ task: "OPP-1", kind: "modified" })).toBe("Edited")
  })
})

describe("tagChangeText", () => {
  it("names the tag and what happened to it", () => {
    expect(tagChangeText({ tag: "bug", kind: "added" })).toBe("Tag bug created")
    expect(tagChangeText({ tag: "server", kind: "modified", renamed_from: "backend" })).toBe(
      "Tag backend renamed to server",
    )
  })
})
