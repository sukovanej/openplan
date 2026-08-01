import { act } from "react"
import { afterEach, describe, expect, it, vi } from "vitest"

import { Tooltip } from "../src/tooltip"
import { render } from "./render"

afterEach(() => vi.useRealTimers())

const enter = (element: Element) => act(() => void element.dispatchEvent(new Event("pointerover", { bubbles: true })))
const leave = (element: Element) => act(() => void element.dispatchEvent(new Event("pointerout", { bubbles: true })))

describe("Tooltip", () => {
  it("shows nothing until the wrapped element is asked about", () => {
    const container = render(
      <Tooltip content="In progress">
        <button>status</button>
      </Tooltip>,
    )
    expect(container.querySelector("[role=tooltip]")).toBeNull()
  })

  it("waits out the pointer sweeping across it", () => {
    vi.useFakeTimers()
    const container = render(
      <Tooltip content="In progress">
        <button>status</button>
      </Tooltip>,
    )
    const anchor = container.firstElementChild!
    enter(anchor)
    expect(container.querySelector("[role=tooltip]")).toBeNull()

    act(() => vi.advanceTimersByTime(300))
    expect(container.querySelector("[role=tooltip]")?.textContent).toBe("In progress")

    leave(anchor)
    expect(container.querySelector("[role=tooltip]")).toBeNull()
  })

  it("stays quiet when a click is what gave the focus", () => {
    const container = render(
      <Tooltip content="In progress">
        <button>status</button>
      </Tooltip>,
    )
    const anchor = container.firstElementChild!
    act(() => void anchor.dispatchEvent(new Event("pointerdown", { bubbles: true })))
    act(() => container.querySelector("button")!.focus())
    expect(container.querySelector("[role=tooltip]")).toBeNull()
  })

  it("answers a keyboard landing on it at once, and names the bubble to the reader", () => {
    const container = render(
      <Tooltip content="Created 14 July 2026">
        <button>2 weeks ago</button>
      </Tooltip>,
    )
    const anchor = container.firstElementChild!
    act(() => container.querySelector("button")!.focus())

    const bubble = container.querySelector("[role=tooltip]")!
    expect(bubble.textContent).toBe("Created 14 July 2026")
    expect(anchor.getAttribute("aria-describedby")).toBe(bubble.id)

    act(() => container.querySelector("button")!.blur())
    expect(container.querySelector("[role=tooltip]")).toBeNull()
  })
})
