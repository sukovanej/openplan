import { act } from "react"
import { describe, expect, it } from "vitest"

import { ParentLink } from "../src/parent-link"
import { render } from "./render"

describe("ParentLink", () => {
  it("links to the parent by key and title", () => {
    const root = render(<ParentLink id="OPP-12" title="Web UI" />)
    const link = root.querySelector("a")!
    expect(link.getAttribute("href")).toBe("/task/OPP-12")
    expect(link.textContent).toBe("OPP-12Web UI")
  })

  it("names the relationship when the keyboard lands on it", () => {
    const root = render(<ParentLink id="OPP-12" title="Web UI" />)
    act(() => root.querySelector("a")!.focus())
    expect(root.querySelector("[role=tooltip]")?.textContent).toBe("Subtask of Web UI")
  })
})
