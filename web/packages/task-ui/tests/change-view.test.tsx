import { describe, expect, it } from "vitest"

import { DocumentChangeView, TagChangeView, TaskChangeView } from "../src/change-view"
import { render } from "./render"

describe("TaskChangeView", () => {
  it("shows a status change as the two status badges and an arrow, and nothing more", () => {
    const shown = render(
      <TaskChangeView
        change={{ task: "OPP-1", kind: "modified", fields: [{ field: "status", from: "todo", to: "in_review" }] }}
      />,
    )
    expect(shown.textContent).toBe("TodoIn review")
    expect(shown.querySelector("[aria-label='to']")).not.toBeNull()
  })

  it("gives every other field an icon beside its words", () => {
    const shown = render(
      <TaskChangeView
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
    expect(render(<TaskChangeView change={{ task: "OPP-1", kind: "added" }} />).textContent).toBe("Created")
    expect(render(<TaskChangeView change={{ task: "OPP-1", kind: "removed" }} />).textContent).toBe("Deleted")
  })
})

describe("TagChangeView and DocumentChangeView", () => {
  it("say what happened, with an icon", () => {
    const renamed = render(<TagChangeView change={{ tag: "server", kind: "modified", renamed_from: "backend" }} />)
    expect(renamed.textContent).toBe("Renamed from backend")
    const edited = render(<DocumentChangeView kind="modified" />)
    expect(edited.textContent).toBe("Edited")
    expect(edited.querySelector("svg")).not.toBeNull()
  })
})
