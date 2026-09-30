import { act, useEffect } from "react"
import { beforeEach, describe, expect, it, vi } from "vitest"

import { HoverCard } from "../src/hover-card"
import { render } from "./render"

const point = (type: string, element: Element, x: number, y: number) =>
  act(() => void element.dispatchEvent(new MouseEvent(type, { bubbles: true, clientX: x, clientY: y })))
const enter = (element: Element, x = 0, y = 0) => point("pointerover", element, x, y)
const move = (element: Element, x: number, y: number) => point("pointermove", element, x, y)
const leave = (element: Element) => point("pointerout", element, 0, 0)
const cards = (container: HTMLElement) => Array.from(container.querySelectorAll<HTMLElement>("[role=dialog]"))
const corner = (open: HTMLElement) => [open.style.left, open.style.top]

const mounted = vi.fn()

function Content() {
  useEffect(() => mounted(), [])
  return <p>the diff</p>
}

const card = (name = "one") => (
  <HoverCard label={`Diff of ${name}`} content={<Content />}>
    <span>{name}</span>
  </HoverCard>
)

describe("HoverCard", () => {
  beforeEach(() => mounted.mockClear())

  it("mounts the content the moment the pointer enters", () => {
    const container = render(card())
    expect(mounted).not.toHaveBeenCalled()
    enter(container.firstElementChild!)
    expect(cards(container).map((open) => open.getAttribute("aria-label"))).toEqual(["Diff of one"])
    expect(mounted).toHaveBeenCalledOnce()
  })

  it("follows the pointer", () => {
    const container = render(card())
    const anchor = container.firstElementChild!
    enter(anchor, 100, 200)
    expect(corner(cards(container)[0])).toEqual(["116px", "216px"])
    move(anchor, 300, 400)
    expect(corner(cards(container)[0])).toEqual(["316px", "416px"])
  })

  it("closes when the pointer leaves", () => {
    const container = render(card())
    const anchor = container.firstElementChild!
    enter(anchor)
    leave(anchor)
    expect(cards(container)).toHaveLength(0)
  })

  it("opens at the anchor for a keyboard, and stays until the focus leaves", () => {
    const container = render(
      <>
        {card()}
        <button>after</button>
      </>,
    )
    const anchor = container.firstElementChild as HTMLElement
    act(() => anchor.focus())
    expect(corner(cards(container)[0])).toEqual(["6px", "6px"])
    move(anchor, 300, 400)
    expect(corner(cards(container)[0])).toEqual(["6px", "6px"])
    leave(anchor)
    expect(cards(container)).toHaveLength(1)
    act(() => cards(container)[0].focus())
    expect(cards(container)).toHaveLength(1)
    act(() => container.querySelector("button")!.focus())
    expect(cards(container)).toHaveLength(0)
  })

  it("closes on Escape", () => {
    const container = render(card())
    const anchor = container.firstElementChild as HTMLElement
    act(() => anchor.focus())
    act(() => void anchor.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true })))
    expect(cards(container)).toHaveLength(0)
  })

  it("keeps one card open at a time", () => {
    const container = render(
      <>
        {card("one")}
        {card("two")}
      </>,
    )
    const [one, two] = Array.from(container.children) as HTMLElement[]
    act(() => one.focus())
    enter(two)
    expect(cards(container).map((open) => open.getAttribute("aria-label"))).toEqual(["Diff of two"])
  })
})
