import { describe, expect, it } from "vitest"

import { TaskIdentity } from "../src/task-identity"
import { TaskRefChip } from "../src/task-ref-chip"
import { render } from "./render"

describe("TaskIdentity", () => {
  it("names a task by its status, key and title", () => {
    const root = render(<TaskIdentity status="in_progress" id="OPP-42" title="Ship login page" />)
    expect(root.querySelector("[aria-label='In progress']")).not.toBeNull()
    expect(root.textContent).toBe("OPP-42Ship login page")
  })

  it("marks a status it could not read rather than showing one", () => {
    const root = render(
      <TaskIdentity status={{ kind: "invalid", message: "not a status" }} id="OPP-7" title="Broken" />,
    )
    expect(root.querySelector("[aria-label='Status could not be read']")).not.toBeNull()
    expect(root.querySelector("[aria-label='Backlog']")).toBeNull()
  })

  it("emphasizes the matched characters of a title it was given indices for", () => {
    const root = render(<TaskIdentity status="todo" id="OPP-3" title="Ship" indices={[0, 1]} />)
    expect(root.querySelector("strong")?.textContent).toBe("Sh")
  })
})

describe("TaskRefChip", () => {
  it("shows the referenced task's status and title", () => {
    const task = { id: "OPP-42", status: "done" as const, title: "Ship login page" }
    const root = render(<TaskRefChip to="/task/OPP-42" id="OPP-42" task={task} />)
    const link = root.querySelector("a")!
    expect(link.getAttribute("href")).toBe("/task/OPP-42")
    expect(link.className).not.toContain("border-dashed")
    expect(link.textContent).toBe("OPP-42Ship login page")
  })

  it("renders a reference it cannot resolve dashed, with its key alone", () => {
    const root = render(<TaskRefChip to="/task/OPP-99" id="OPP-99" task={undefined} />)
    const link = root.querySelector("a")!
    expect(link.className).toContain("border-dashed")
    expect(link.textContent).toBe("OPP-99")
  })
})
