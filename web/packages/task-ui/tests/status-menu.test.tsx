import { act } from "react"
import { describe, expect, it } from "vitest"

import type { Status } from "@openplan/api-client"

import { STATUSES } from "../src/status"
import { StatusMenu } from "../src/status-menu"
import { render } from "./render"

const options = (container: HTMLElement) => [...container.querySelectorAll("[role='option']")]

const press = (container: HTMLElement, key: string) => {
  const list = container.querySelector("[role='listbox']")!
  act(() => {
    list.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true }))
  })
}

const noop = () => {}

describe("StatusMenu", () => {
  it("offers every status the board groups by, in that order", () => {
    const root = render(<StatusMenu current="todo" onPick={noop} onClose={noop} />)
    expect(options(root).map((option) => option.textContent)).toEqual([
      "Backlog",
      "Todo",
      "In progress",
      "In review",
      "Done",
      "Cancelled",
    ])
    expect(STATUSES).toEqual(["backlog", "todo", "in_progress", "in_review", "done", "cancelled"])
  })

  it("opens on the status the task carries", () => {
    const root = render(<StatusMenu current="done" onPick={noop} onClose={noop} />)
    const active = options(root).filter((option) => option.getAttribute("aria-selected") === "true")
    expect(active.map((option) => option.textContent)).toEqual(["Done"])
  })

  it("opens on the first status when the task carries none that could be read", () => {
    const root = render(<StatusMenu current={undefined} onPick={noop} onClose={noop} />)
    expect(options(root)[0].getAttribute("aria-selected")).toBe("true")
  })

  it("reports the status that was clicked", () => {
    const picked: Array<Status> = []
    const root = render(<StatusMenu current="todo" onPick={(status) => picked.push(status)} onClose={noop} />)
    act(() => {
      options(root)[4].dispatchEvent(new MouseEvent("mousedown", { bubbles: true, cancelable: true }))
    })
    expect(picked).toEqual(["done"])
  })

  it("walks the list with the arrow keys and picks with Enter", () => {
    const picked: Array<Status> = []
    const root = render(<StatusMenu current="backlog" onPick={(status) => picked.push(status)} onClose={noop} />)
    press(root, "ArrowDown")
    press(root, "ArrowDown")
    press(root, "ArrowUp")
    press(root, "Enter")
    expect(picked).toEqual(["todo"])
  })

  it("wraps around at both ends", () => {
    const picked: Array<Status> = []
    const root = render(<StatusMenu current="backlog" onPick={(status) => picked.push(status)} onClose={noop} />)
    press(root, "ArrowUp")
    press(root, "Enter")
    expect(picked).toEqual(["cancelled"])
  })

  it("closes on Escape without picking anything", () => {
    let closed = 0
    const picked: Array<Status> = []
    const root = render(<StatusMenu current="todo" onPick={(status) => picked.push(status)} onClose={() => closed++} />)
    press(root, "Escape")
    expect(closed).toBe(1)
    expect(picked).toEqual([])
  })

  // The list holds the focus while it is open, so the app's own single-key bindings must not fire
  // under it — `data-keys-ignore` is what the dispatcher reads to stay off them.
  it("keeps the page's single-key bindings off the keys it answers", () => {
    const root = render(<StatusMenu current="todo" onPick={noop} onClose={noop} />)
    expect(root.querySelector("[role='listbox']")?.hasAttribute("data-keys-ignore")).toBe(true)
  })
})
