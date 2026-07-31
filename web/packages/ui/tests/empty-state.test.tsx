import { describe, expect, it } from "vitest"

import { EmptyState } from "../src/empty-state"
import { render } from "./render"

describe("EmptyState", () => {
  it("renders the title on its own", () => {
    const state = render(<EmptyState title="No tasks yet" />)
    expect(state.querySelectorAll("p")).toHaveLength(1)
    expect(state.textContent).toBe("No tasks yet")
  })

  it("adds the detail below the title when there is one", () => {
    const paragraphs = [...render(<EmptyState title="Could not load tasks" detail="offline" />).querySelectorAll("p")]
    expect(paragraphs.map((p) => p.textContent)).toEqual(["Could not load tasks", "offline"])
  })
})
