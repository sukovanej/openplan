import { describe, expect, it } from "vitest"

import type { TagView } from "@openplan/api-client"

import { DocumentChangeView, TagChangeView, TaskChangeView } from "../src/change-view"
import { render } from "./render"

describe("TaskChangeView", () => {
  it("shows a status change as the two status badges and an arrow, and nothing more", () => {
    const shown = render(
      <TaskChangeView
        tags={undefined}
        change={{ task: "OPP-1", kind: "modified", fields: [{ field: "status", from: "todo", to: "in_review" }] }}
      />,
    )
    expect(shown.textContent).toBe("TodoIn review")
    expect(shown.querySelector("[aria-label='to']")).not.toBeNull()
  })

  it("gives every other field an icon beside its words", () => {
    const shown = render(
      <TaskChangeView
        tags={undefined}
        change={{
          task: "OPP-1",
          kind: "modified",
          fields: [{ field: "description" }, { field: "comments", added: 2, removed: 0 }],
        }}
      />,
    )
    expect(shown.textContent).toBe("Description2 comments")
    expect(shown.querySelectorAll("svg[aria-hidden='true']")).toHaveLength(2)
  })

  it("names a task that came or went", () => {
    expect(render(<TaskChangeView change={{ task: "OPP-1", kind: "added" }} tags={undefined} />).textContent).toBe(
      "Created",
    )
    expect(render(<TaskChangeView change={{ task: "OPP-1", kind: "removed" }} tags={undefined} />).textContent).toBe(
      "Deleted",
    )
  })
})

describe("TagChangeView and DocumentChangeView", () => {
  it("say what happened, with an icon", () => {
    const renamed = render(
      <TagChangeView change={{ tag: "server", kind: "modified", renamed_from: "backend" }} tags={undefined} />,
    )
    expect(renamed.textContent).toBe("Renamed from backend")
    const edited = render(<DocumentChangeView kind="modified" />)
    expect(edited.textContent).toBe("Edited")
    expect(edited.querySelector("svg")).not.toBeNull()
  })
})

const registry: ReadonlyMap<string, TagView> = new Map([
  ["bug", { name: "bug", display: "Bug", color: "red" }],
  ["server", { name: "server", display: "Server", color: "blue" }],
])

const chips = (container: HTMLElement) =>
  [...container.querySelectorAll("span.rounded-md")].map((chip) => ({
    text: chip.textContent,
    dangling: chip.classList.contains("border-dashed"),
  }))

describe("tag changes with the registry", () => {
  it("show each tag that joined or left a task as a chip that carries its sign", () => {
    const shown = render(
      <TaskChangeView
        tags={registry}
        change={{ task: "OPP-1", kind: "modified", fields: [{ field: "tags", from: ["gone", "x"], to: ["x", "bug"] }] }}
      />,
    )
    expect(shown.textContent).toBe("+Bug−gone")
    expect(chips(shown)).toEqual([
      { text: "+Bug", dangling: false },
      { text: "−gone", dangling: true },
    ])
  })

  it("keep the words for tags that only changed order", () => {
    const shown = render(
      <TaskChangeView
        tags={registry}
        change={{ task: "OPP-1", kind: "modified", fields: [{ field: "tags", from: ["a", "b"], to: ["b", "a"] }] }}
      />,
    )
    expect(shown.textContent).toBe("Tags reordered")
  })

  it("show a rename as the old name and the new name, both as chips", () => {
    const shown = render(
      <TagChangeView tags={registry} change={{ tag: "server", kind: "modified", renamed_from: "backend" }} />,
    )
    expect(shown.textContent).toBe("RenamedbackendServer")
    expect(chips(shown)).toEqual([
      { text: "backend", dangling: true },
      { text: "Server", dangling: false },
    ])
  })
})
