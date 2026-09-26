import { act } from "react"
import { describe, expect, it } from "vitest"

import { DocParentLink, ParentLink } from "../src/parent-link"
import { render } from "./render"

describe("ParentLink", () => {
  it("links to the parent by key and title", () => {
    const root = render(<ParentLink project="openplan" id="OPP-12" title="Web UI" />)
    const link = root.querySelector("a")!
    expect(link.getAttribute("href")).toBe("/openplan/task/OPP-12")
    expect(link.textContent).toBe("OPP-12Web UI")
  })

  it("names the relationship when the keyboard lands on it", () => {
    const root = render(<ParentLink project="openplan" id="OPP-12" title="Web UI" />)
    act(() => root.querySelector("a")!.focus())
    expect(root.querySelector("[role=tooltip]")?.textContent).toBe("Subtask of Web UI")
  })
})

describe("DocParentLink", () => {
  it("links to the parent doc by its title", () => {
    const root = render(<DocParentLink project="openplan" name="guides" title="Guides" />)
    const link = root.querySelector("a")!
    expect(link.getAttribute("href")).toBe("/openplan/doc/guides")
    expect(link.textContent).toBe("Guides")
  })

  it("names the relationship when the keyboard lands on it", () => {
    const root = render(<DocParentLink project="openplan" name="guides" title="Guides" />)
    act(() => root.querySelector("a")!.focus())
    expect(root.querySelector("[role=tooltip]")?.textContent).toBe("Nested under Guides")
  })
})
