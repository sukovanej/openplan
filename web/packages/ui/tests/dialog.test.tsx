import { act, useState } from "react"
import { describe, expect, it, vi } from "vitest"

import { Dialog } from "../src/dialog"
import { render } from "./render"

const tab = (node: HTMLElement, shiftKey = false) =>
  act(() => {
    node.dispatchEvent(new KeyboardEvent("keydown", { key: "Tab", shiftKey, bubbles: true }))
  })

const dialogOf = (container: HTMLElement) => container.querySelector<HTMLElement>('[role="dialog"]')!

describe("Dialog", () => {
  it("renders nothing until it is open", () => {
    expect(
      render(
        <Dialog open={false} onClose={() => {}} title="Shortcuts">
          <p>body</p>
        </Dialog>,
      ).textContent,
    ).toBe("")
  })

  it("takes focus when it opens", () => {
    const container = render(
      <Dialog open onClose={() => {}} title="Shortcuts">
        <p>body</p>
      </Dialog>,
    )
    expect(document.activeElement).toBe(dialogOf(container))
  })

  it("returns focus to whatever opened it", () => {
    function Harness() {
      const [open, setOpen] = useState(false)
      return (
        <>
          <button type="button" onClick={() => setOpen(true)}>
            open
          </button>
          <Dialog open={open} onClose={() => setOpen(false)} title="Shortcuts">
            <p>body</p>
          </Dialog>
        </>
      )
    }
    const container = render(<Harness />)
    const opener = container.querySelector("button")!

    act(() => opener.focus())
    act(() => opener.click())
    expect(document.activeElement).toBe(dialogOf(container))

    act(() => {
      container.querySelector<HTMLElement>('[aria-label="Close"]')!.click()
    })
    expect(container.querySelector('[role="dialog"]')).toBeNull()
    expect(document.activeElement).toBe(opener)
  })

  it("wraps Tab from the last focusable back to the first", () => {
    const container = render(
      <Dialog open onClose={() => {}} title="Shortcuts">
        <button type="button">last</button>
      </Dialog>,
    )
    const dialog = dialogOf(container)
    const [close, last] = [...dialog.querySelectorAll("button")]

    act(() => last.focus())
    tab(dialog)
    expect(document.activeElement).toBe(close)
  })

  it("wraps Shift+Tab from the dialog itself to the last focusable", () => {
    const container = render(
      <Dialog open onClose={() => {}} title="Shortcuts">
        <button type="button">last</button>
      </Dialog>,
    )
    const dialog = dialogOf(container)
    const buttons = [...dialog.querySelectorAll("button")]

    tab(dialog, true)
    expect(document.activeElement).toBe(buttons[buttons.length - 1])
  })

  it("closes on the backdrop but not on the dialog itself", () => {
    const onClose = vi.fn()
    const container = render(
      <Dialog open onClose={onClose} title="Shortcuts">
        <p>body</p>
      </Dialog>,
    )
    act(() => {
      dialogOf(container).click()
    })
    expect(onClose).not.toHaveBeenCalled()

    act(() => {
      container.querySelector<HTMLElement>('[role="presentation"]')!.click()
    })
    expect(onClose).toHaveBeenCalledOnce()
  })

  it("closes from its own close button", () => {
    const onClose = vi.fn()
    const container = render(
      <Dialog open onClose={onClose} title="Shortcuts">
        <p>body</p>
      </Dialog>,
    )
    act(() => {
      container.querySelector<HTMLElement>('[aria-label="Close"]')!.click()
    })
    expect(onClose).toHaveBeenCalledOnce()
  })
})
