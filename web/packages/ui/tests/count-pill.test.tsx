import { describe, expect, it } from "vitest"

import { CountPill } from "../src/count-pill"
import { render } from "./render"

describe("CountPill", () => {
  it("shows the count", () => {
    expect(render(<CountPill count={7} />).textContent).toBe("7")
  })

  it("shows a zero rather than nothing, so the caller decides when to hide it", () => {
    expect(render(<CountPill count={0} />).textContent).toBe("0")
  })
})
