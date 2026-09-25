import { act, useRef, useState } from "react"
import { describe, expect, it, vi } from "vitest"

import { Popover } from "../src/popover"
import { render } from "./render"

function Anchored({ onClose }: { onClose: () => void }) {
  const anchor = useRef<HTMLButtonElement>(null)
  const [open, setOpen] = useState(true)
  return (
    <>
      <button ref={anchor} type="button" onClick={() => setOpen(!open)}>
        anchor
      </button>
      <span>outside</span>
      {open && (
        <Popover
          anchor={anchor}
          label="Choices"
          onClose={() => {
            setOpen(false)
            onClose()
          }}
        >
          <button type="button">inside</button>
        </Popover>
      )}
    </>
  )
}

const mouseDown = (target: Element) =>
  act(() => void target.dispatchEvent(new MouseEvent("mousedown", { bubbles: true })))

const button = (container: HTMLElement, name: string) =>
  [...container.querySelectorAll("button")].find((each) => each.textContent === name)!

describe("Popover", () => {
  it("opens as a named dialog that holds the focus", () => {
    const container = render(<Anchored onClose={vi.fn()} />)
    const dialog = container.querySelector("[role=dialog]")
    expect(dialog?.getAttribute("aria-label")).toBe("Choices")
    expect(document.activeElement).toBe(dialog)
  })

  it("closes on a press outside both itself and its anchor", () => {
    const onClose = vi.fn()
    const container = render(<Anchored onClose={onClose} />)
    mouseDown(button(container, "inside"))
    mouseDown(button(container, "anchor"))
    expect(onClose).not.toHaveBeenCalled()
    mouseDown(container.querySelector("span")!)
    expect(onClose).toHaveBeenCalledTimes(1)
    expect(container.querySelector("[role=dialog]")).toBeNull()
  })

  it("closes on Escape, marks the key handled, and gives the focus back to its anchor", () => {
    const onClose = vi.fn()
    const container = render(<Anchored onClose={onClose} />)
    const escape = new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true })
    act(() => void container.querySelector("[role=dialog]")!.dispatchEvent(escape))
    expect(onClose).toHaveBeenCalledTimes(1)
    expect(escape.defaultPrevented).toBe(true)
    expect(document.activeElement).toBe(button(container, "anchor"))
  })

  it("keeps the app's single-key bindings off the keys pressed inside it", () => {
    const container = render(<Anchored onClose={vi.fn()} />)
    expect(container.querySelector("[role=dialog]")?.hasAttribute("data-keys-ignore")).toBe(true)
  })
})
