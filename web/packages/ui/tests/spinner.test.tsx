import { describe, expect, it } from "vitest"

import { Spinner } from "../src/spinner"
import { render } from "./render"

describe("Spinner", () => {
  it("is a status that names what it waits for", () => {
    const status = render(<Spinner label="Publishing" />).querySelector('[role="status"]')!
    expect(status.getAttribute("aria-label")).toBe("Publishing")
    expect(status.classList.contains("animate-spin")).toBe(true)
  })

  it("takes a size from its caller", () => {
    const status = render(<Spinner label="Starting" className="size-3" />).querySelector('[role="status"]')!
    expect(status.classList.contains("size-3")).toBe(true)
    expect(status.classList.contains("size-4")).toBe(false)
  })
})
