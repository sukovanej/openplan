import { describe, expect, it } from "vitest"

import { Kbd } from "../src/kbd"
import { render } from "./render"

describe("Kbd", () => {
  it("puts a plus between a modifier and its key", () => {
    expect(render(<Kbd token="mod+k" />).textContent).toMatch(/^(⌘|Ctrl) \+ K$/)
    expect(render(<Kbd token="mod+." />).textContent).toMatch(/^(⌘|Ctrl) \+ \.$/)
  })

  it("separates every modifier of a chord", () => {
    expect(render(<Kbd token="mod+shift+Enter" />).textContent).toMatch(/^(⌘|Ctrl) \+ (⇧|Shift) \+ ↵$/)
  })

  it("leaves a bare key alone", () => {
    expect(render(<Kbd token="j" />).textContent).toBe("J")
  })
})
