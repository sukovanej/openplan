import { act } from "react"
import { describe, expect, it } from "vitest"

import type { Problem } from "@openplan/api-client"

import { ProblemBadge, ProblemBanner } from "../src/problem-mark"
import { render } from "./render"

const MISSING_PARENT: Problem = { code: "reference", message: "the parent OPP-9 does not exist" }
const WAITS_FOR_ITSELF: Problem = {
  code: "dependency_cycle",
  message: "the task waits for itself: OPP-3 → OPP-4 → OPP-3",
}

const badge = (container: HTMLElement) => container.querySelector<HTMLElement>("[tabindex='0']")!

describe("ProblemBadge", () => {
  it("counts one problem in the singular", () => {
    expect(badge(render(<ProblemBadge problems={[MISSING_PARENT]} />)).textContent).toBe("1 problem")
  })

  it("counts more than one problem in the plural", () => {
    const root = render(<ProblemBadge problems={[MISSING_PARENT, WAITS_FOR_ITSELF]} />)
    expect(badge(root).textContent).toBe("2 problems")
  })

  it("wears the failure tone, not the conflict tone", () => {
    const { className } = badge(render(<ProblemBadge problems={[MISSING_PARENT]} />))
    expect(className).toContain("text-danger")
    expect(className).not.toContain("text-warning")
  })

  it("lists each problem when the keyboard lands on it", () => {
    const root = render(<ProblemBadge problems={[MISSING_PARENT, WAITS_FOR_ITSELF]} />)
    act(() => badge(root).focus())
    const messages = [...root.querySelectorAll("[role=tooltip] li")].map((item) => item.textContent)
    expect(messages).toEqual([MISSING_PARENT.message, WAITS_FOR_ITSELF.message])
  })
})

describe("ProblemBanner", () => {
  it("shows nothing for a task without problems", () => {
    expect(render(<ProblemBanner problems={[]} />).innerHTML).toBe("")
  })

  it("counts the problems and lists each message in order", () => {
    const root = render(<ProblemBanner problems={[MISSING_PARENT, WAITS_FOR_ITSELF]} />)
    const note = root.querySelector("[role=note]")!
    expect(note.querySelector("p")?.textContent).toBe("2 problems in this task.")
    const messages = [...note.querySelectorAll("li")].map((item) => item.textContent)
    expect(messages).toEqual([MISSING_PARENT.message, WAITS_FOR_ITSELF.message])
  })

  it("wears the failure tone, not the conflict tone", () => {
    const { className } = render(<ProblemBanner problems={[MISSING_PARENT]} />).querySelector("[role=note]")!
    expect(className).toContain("border-danger/40")
    expect(className).not.toContain("warning")
  })
})
