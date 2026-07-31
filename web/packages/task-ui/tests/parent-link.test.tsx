import { describe, expect, it } from "vitest"

import { ParentLink } from "../src/parent-link"
import { render } from "./render"

describe("ParentLink", () => {
  it("links to the parent by key and title", () => {
    const root = render(<ParentLink id="OPP-12" title="Web UI" />)
    const link = root.querySelector("a")!
    expect(link.getAttribute("href")).toBe("/task/OPP-12")
    expect(link.textContent).toBe("OPP-12Web UI")
    expect(root.querySelector("[title='Subtask of Web UI']")).not.toBeNull()
  })
})
