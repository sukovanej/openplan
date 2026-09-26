import { act, useEffect } from "react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import { HoverCard } from "../src/hover-card"
import { render } from "./render"

beforeEach(() => vi.useFakeTimers())
afterEach(() => vi.useRealTimers())

const enter = (element: Element) => act(() => void element.dispatchEvent(new Event("pointerover", { bubbles: true })))
const leave = (element: Element) => act(() => void element.dispatchEvent(new Event("pointerout", { bubbles: true })))
const wait = (ms: number) => act(() => void vi.advanceTimersByTime(ms))
const cards = (container: HTMLElement) => Array.from(container.querySelectorAll("[role=dialog]"))

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

  it("mounts the content only when the hover delay ends", () => {
    const container = render(card())
    const anchor = container.firstElementChild!
    enter(anchor)
    wait(299)
    expect(mounted).not.toHaveBeenCalled()
    wait(1)
    expect(cards(container).map((open) => open.getAttribute("aria-label"))).toEqual(["Diff of one"])
    expect(mounted).toHaveBeenCalledOnce()
  })

  it("mounts nothing for a pointer that sweeps across", () => {
    const container = render(card())
    const anchor = container.firstElementChild!
    enter(anchor)
    wait(100)
    leave(anchor)
    wait(1000)
    expect(mounted).not.toHaveBeenCalled()
  })

  it("stays open while the pointer crosses to the card", () => {
    const container = render(card())
    const anchor = container.firstElementChild!
    enter(anchor)
    wait(300)
    leave(anchor)
    wait(100)
    enter(cards(container)[0])
    wait(1000)
    expect(cards(container)).toHaveLength(1)
  })

  it("closes a moment after the pointer leaves", () => {
    const container = render(card())
    const anchor = container.firstElementChild!
    enter(anchor)
    wait(300)
    leave(anchor)
    wait(150)
    expect(cards(container)).toHaveLength(0)
  })

  it("opens at once for a keyboard, and stays until the focus leaves", () => {
    const container = render(
      <>
        {card()}
        <button>after</button>
      </>,
    )
    const anchor = container.firstElementChild as HTMLElement
    act(() => anchor.focus())
    expect(cards(container)).toHaveLength(1)
    leave(anchor)
    wait(1000)
    expect(cards(container)).toHaveLength(1)
    act(() => (cards(container)[0] as HTMLElement).focus())
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
    wait(300)
    expect(cards(container).map((open) => open.getAttribute("aria-label"))).toEqual(["Diff of two"])
  })
})
