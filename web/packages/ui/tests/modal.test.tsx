import { act, useState } from "react"
import { describe, expect, it, vi } from "vitest"

import { Modal } from "../src/modal"
import { render } from "./render"

const tab = (node: HTMLElement, shiftKey = false) =>
  act(() => {
    node.dispatchEvent(new KeyboardEvent("keydown", { key: "Tab", shiftKey, bubbles: true }))
  })

const dialogOf = () => document.querySelector<HTMLElement>('[role="dialog"]')!

describe("Modal", () => {
  it("renders nothing until it is open", () => {
    render(
      <Modal open={false} onClose={() => {}} label="Shortcuts">
        <p>body</p>
      </Modal>,
    )
    expect(document.querySelector('[role="dialog"]')).toBeNull()
  })

  it("renders under the body, outside the content that opens it", () => {
    const container = render(
      <Modal open onClose={() => {}} label="Shortcuts">
        <p>body</p>
      </Modal>,
    )
    expect(container.contains(dialogOf())).toBe(false)
    expect(document.body.contains(dialogOf())).toBe(true)
  })

  it("names the dialog by its label", () => {
    render(
      <Modal open onClose={() => {}} label="Shortcuts">
        <p>body</p>
      </Modal>,
    )
    expect(dialogOf().getAttribute("aria-label")).toBe("Shortcuts")
  })

  it("takes focus when it opens", () => {
    render(
      <Modal open onClose={() => {}} label="Shortcuts">
        <p>body</p>
      </Modal>,
    )
    expect(document.activeElement).toBe(dialogOf())
  })

  it("returns focus to whatever opened it", () => {
    function Harness() {
      const [open, setOpen] = useState(false)
      return (
        <>
          <button type="button" onClick={() => setOpen(true)}>
            open
          </button>
          <Modal open={open} onClose={() => setOpen(false)} label="Shortcuts">
            <button type="button" onClick={() => setOpen(false)}>
              done
            </button>
          </Modal>
        </>
      )
    }
    const container = render(<Harness />)
    const opener = container.querySelector("button")!

    act(() => opener.focus())
    act(() => opener.click())
    expect(document.activeElement).toBe(dialogOf())

    act(() => {
      dialogOf().querySelector("button")!.click()
    })
    expect(document.querySelector('[role="dialog"]')).toBeNull()
    expect(document.activeElement).toBe(opener)
  })

  it("wraps Tab from the last focusable back to the first", () => {
    render(
      <Modal open onClose={() => {}} label="Shortcuts">
        <button type="button">first</button>
        <button type="button">last</button>
      </Modal>,
    )
    const dialog = dialogOf()
    const [first, last] = [...dialog.querySelectorAll("button")]

    act(() => last.focus())
    tab(dialog)
    expect(document.activeElement).toBe(first)
  })

  it("wraps Shift+Tab from the dialog itself to the last focusable", () => {
    render(
      <Modal open onClose={() => {}} label="Shortcuts">
        <button type="button">first</button>
        <button type="button">last</button>
      </Modal>,
    )
    const dialog = dialogOf()
    const buttons = [...dialog.querySelectorAll("button")]

    tab(dialog, true)
    expect(document.activeElement).toBe(buttons[buttons.length - 1])
  })

  it("closes on the backdrop but not on the dialog itself", () => {
    const onClose = vi.fn()
    render(
      <Modal open onClose={onClose} label="Shortcuts">
        <p>body</p>
      </Modal>,
    )
    act(() => {
      dialogOf().click()
    })
    expect(onClose).not.toHaveBeenCalled()

    act(() => {
      document.querySelector<HTMLElement>('[role="presentation"]')!.click()
    })
    expect(onClose).toHaveBeenCalledOnce()
  })

  // The app's key dispatcher skips a handled key, so Esc must not also run the page's own binding.
  it("closes on Escape and marks the key handled", () => {
    const onClose = vi.fn()
    render(
      <Modal open onClose={onClose} label="Shortcuts">
        <p>body</p>
      </Modal>,
    )
    const escape = new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true })
    act(() => {
      dialogOf().dispatchEvent(escape)
    })
    expect(onClose).toHaveBeenCalledOnce()
    expect(escape.defaultPrevented).toBe(true)
  })

  it("passes its other attributes to the dialog", () => {
    render(
      <Modal open onClose={() => {}} label="Shortcuts" data-keys-ignore>
        <p>body</p>
      </Modal>,
    )
    expect(dialogOf().hasAttribute("data-keys-ignore")).toBe(true)
  })
})
