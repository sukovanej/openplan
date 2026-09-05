import { act, useRef } from "react"
import { describe, expect, it, vi } from "vitest"

import { useDismissOnOutsideClick } from "../src/use-dismiss"
import { render } from "./render"

function Popover({ onDismiss }: { onDismiss?: () => void }) {
  const root = useRef<HTMLDivElement>(null)
  useDismissOnOutsideClick(root, onDismiss)
  return (
    <div ref={root}>
      <button>inside</button>
    </div>
  )
}

const mouseDown = (target: Element) =>
  act(() => void target.dispatchEvent(new MouseEvent("mousedown", { bubbles: true })))

describe("useDismissOnOutsideClick", () => {
  it("dismisses on a press outside the watched element", () => {
    const onDismiss = vi.fn()
    render(<Popover onDismiss={onDismiss} />)
    mouseDown(document.body)
    expect(onDismiss).toHaveBeenCalledTimes(1)
  })

  it("keeps a press inside the watched element", () => {
    const onDismiss = vi.fn()
    const container = render(<Popover onDismiss={onDismiss} />)
    mouseDown(container.querySelector("button")!)
    expect(onDismiss).not.toHaveBeenCalled()
  })

  it("listens for nothing while the caller has no dismissal", () => {
    const add = vi.spyOn(document, "addEventListener")
    render(<Popover />)
    expect(add.mock.calls.filter(([type]) => type === "mousedown")).toHaveLength(0)
    add.mockRestore()
  })
})
