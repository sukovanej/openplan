import { act } from "react"
import { describe, expect, it, vi } from "vitest"

import { Dialog } from "../src/dialog"
import { render } from "./render"

const dialogOf = () => document.querySelector<HTMLElement>('[role="dialog"]')!

describe("Dialog", () => {
  it("heads the modal with its title and names it by the title", () => {
    render(
      <Dialog open onClose={() => {}} title="Shortcuts">
        <p>body</p>
      </Dialog>,
    )
    expect(dialogOf().querySelector("h2")!.textContent).toBe("Shortcuts")
    expect(dialogOf().getAttribute("aria-label")).toBe("Shortcuts")
  })

  it("closes from its own close button", () => {
    const onClose = vi.fn()
    render(
      <Dialog open onClose={onClose} title="Shortcuts">
        <p>body</p>
      </Dialog>,
    )
    act(() => {
      document.querySelector<HTMLElement>('[aria-label="Close"]')!.click()
    })
    expect(onClose).toHaveBeenCalledOnce()
  })
})
