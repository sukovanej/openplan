import { describe, expect, it } from "vitest"

import { DocRefChip } from "../src/doc-ref-chip"
import { RefReader } from "../src/ref-reader"
import { TaskIdentity } from "../src/task-identity"
import { TaskRefChip } from "../src/task-ref-chip"
import { holding } from "./refs"
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

const store = holding({ "OPP-42": "done" }, ["storage"])

describe("TaskRefChip", () => {
  it("shows the status and the key of the task it reads", () => {
    const root = render(
      <RefReader value={store}>
        <TaskRefChip to="/openplan/task/OPP-42" project="openplan" id="OPP-42" />
      </RefReader>,
    )
    const link = root.querySelector("a")!
    expect(link.getAttribute("href")).toBe("/openplan/task/OPP-42")
    expect(link.className).not.toContain("border-dashed")
    expect(link.querySelector("[aria-label='Done']")).not.toBeNull()
    expect(link.textContent).toBe("OPP-42")
  })

  it("renders a task the store does not hold dashed", () => {
    const root = render(
      <RefReader value={store}>
        <TaskRefChip to="/openplan/task/OPP-99" project="openplan" id="OPP-99" />
      </RefReader>,
    )
    const link = root.querySelector("a")!
    expect(link.className).toContain("border-dashed")
    expect(link.textContent).toBe("OPP-99")
  })

  it("claims neither a status nor a missing task while its task is being read", () => {
    const link = render(<TaskRefChip to="/openplan/task/OPP-2" project="openplan" id="OPP-2" sign="+" />).querySelector(
      "a",
    )!
    expect(link.textContent).toBe("+OPP-2")
    expect(link.className).not.toContain("border-dashed")
    expect(link.querySelector("svg")).toBeNull()
  })
})

describe("DocRefChip", () => {
  it("shows the name of the doc, and renders a doc the store does not hold dashed", () => {
    const root = render(
      <RefReader value={store}>
        <DocRefChip to="/openplan/doc/storage" project="openplan" name="storage" />
        <DocRefChip to="/openplan/doc/gone" project="openplan" name="gone" />
      </RefReader>,
    )
    expect(
      [...root.querySelectorAll("a")].map((link) => [link.textContent, link.className.includes("border-dashed")]),
    ).toEqual([
      ["storage", false],
      ["gone", true],
    ])
  })
})
