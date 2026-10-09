import { afterEach, describe, expect, it, vi } from "vitest"

import { Kbd } from "../src/kbd"
import { render } from "./render"

describe("Kbd", () => {
  it("puts a plus between a modifier and its key", () => {
    expect(render(<Kbd token="mod+k" />).textContent).toMatch(/^(⌘|Ctrl)\+K$/)
    expect(render(<Kbd token="mod+." />).textContent).toMatch(/^(⌘|Ctrl)\+\.$/)
  })

  it("separates every modifier of a chord", () => {
    expect(render(<Kbd token="mod+shift+Enter" />).textContent).toMatch(/^(⌘|Ctrl)\+(⇧|Shift)\+⏎$/)
  })

  it("leaves a bare key alone", () => {
    expect(render(<Kbd token="j" />).textContent).toBe("J")
  })

  describe("on a Mac", () => {
    afterEach(() => {
      vi.unstubAllGlobals()
      vi.resetModules()
    })

    it("writes a chord as its symbols side by side, in the order macOS uses", async () => {
      vi.stubGlobal("navigator", { userAgent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 15_0)" })
      vi.resetModules()
      const { Kbd: MacKbd } = await import("../src/kbd")
      expect(render(<MacKbd token="mod+shift+Enter" />).textContent).toBe("⇧⌘⏎")
      expect(render(<MacKbd token="mod+k" />).textContent).toBe("⌘K")
    })
  })
})
