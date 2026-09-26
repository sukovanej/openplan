import { act } from "react"
import { useLocation } from "react-router-dom"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import { DiagramViewport } from "../src/diagram-viewport"
import { render } from "./render"

const drawing = {
  svg: '<svg class="op-diagram" width="400" height="200"><a href="/openplan/task/OPP-2"><g class="node"><text>Card</text></g></a><a href="https://example.com"><text>Out</text></a></svg>',
  width: 400,
  height: 200,
}

function Where() {
  return <output data-testid="where">{useLocation().pathname}</output>
}

async function mounted(): Promise<{ frame: HTMLElement; layer: HTMLElement; root: HTMLElement }> {
  // happy-dom lays nothing out, so the frame gets the size a page would give it.
  vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(800)
  vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(600)
  const root = render(
    <>
      <DiagramViewport drawing={drawing} label="Flow" fitKey="first" />
      <Where />
    </>,
  )
  await act(async () => {
    await Promise.resolve()
  })
  const frame = root.querySelector<HTMLElement>("[role='group'][aria-label='Flow']")!
  return { frame, layer: frame.firstElementChild as HTMLElement, root }
}

const scaleOf = (layer: HTMLElement) => Number(/scale\(([^)]+)\)/.exec(layer.style.transform)![1])
const translateOf = (layer: HTMLElement) =>
  /translate\(([-\d.]+)px, ([-\d.]+)px\)/.exec(layer.style.transform)!.slice(1).map(Number)

function wheel(frame: HTMLElement, init: WheelEventInit): WheelEvent {
  const event = new WheelEvent("wheel", { bubbles: true, cancelable: true, ...init })
  // happy-dom drops the modifier keys of a WheelEvent init.
  Object.defineProperty(event, "ctrlKey", { value: init.ctrlKey ?? false })
  act(() => {
    frame.dispatchEvent(event)
  })
  return event
}

function pointer(frame: HTMLElement, type: string, x: number, y: number): void {
  act(() => {
    frame.dispatchEvent(
      new PointerEvent(type, { bubbles: true, pointerId: 1, pointerType: "mouse", button: 0, clientX: x, clientY: y }),
    )
  })
}

beforeEach(() => {
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] })
})

afterEach(() => {
  vi.useRealTimers()
  vi.restoreAllMocks()
})

describe("the diagram viewport", () => {
  it("fits the drawing to the page when it opens, up to its own size", async () => {
    const { layer } = await mounted()
    expect(scaleOf(layer)).toBe(1)
    expect(translateOf(layer)).toEqual([200, 200])
  })

  it("pans on the wheel and zooms on the wheel with Ctrl", async () => {
    const { frame, layer } = await mounted()
    const pan = wheel(frame, { deltaX: 10, deltaY: 30 })
    expect(pan.defaultPrevented).toBe(true)
    expect(translateOf(layer)).toEqual([190, 170])
    expect(scaleOf(layer)).toBe(1)

    wheel(frame, { deltaY: -20, ctrlKey: true })
    expect(scaleOf(layer)).toBeCloseTo(Math.exp(0.2))
  })

  it("keeps one Ctrl wheel notch from jumping far", async () => {
    const { frame, layer } = await mounted()
    wheel(frame, { deltaY: -500, ctrlKey: true })
    expect(scaleOf(layer)).toBeCloseTo(Math.exp(0.5))
  })

  it("holds a compositor layer only while the gesture lasts", async () => {
    const { frame, layer } = await mounted()
    wheel(frame, { deltaY: 30 })
    expect(layer.style.willChange).toBe("transform")
    act(() => {
      vi.advanceTimersByTime(149)
    })
    expect(layer.style.willChange).toBe("transform")
    act(() => {
      vi.advanceTimersByTime(1)
    })
    expect(layer.style.willChange).toBe("")
  })

  it("zooms with the buttons and fits again", async () => {
    const { frame, layer } = await mounted()
    act(() => frame.querySelector<HTMLElement>("[aria-label='Zoom in']")!.click())
    expect(scaleOf(layer)).toBeCloseTo(1.25)
    act(() => frame.querySelector<HTMLElement>("[aria-label='Fit to the page']")!.click())
    expect(scaleOf(layer)).toBe(1)
    expect(translateOf(layer)).toEqual([200, 200])
  })

  it("opens a card of this app in place", async () => {
    const { frame, root } = await mounted()
    const card = frame.querySelector("a[href^='/'] text")!
    const click = new MouseEvent("click", { bubbles: true, cancelable: true, button: 0 })
    act(() => {
      card.dispatchEvent(click)
    })
    expect(click.defaultPrevented).toBe(true)
    expect(root.querySelector("[data-testid='where']")!.textContent).toBe("/openplan/task/OPP-2")
  })

  it("leaves a link to another site to the browser", async () => {
    const { frame, root } = await mounted()
    const click = new MouseEvent("click", { bubbles: true, cancelable: true, button: 0 })
    act(() => {
      frame.querySelector("a[href^='https'] text")!.dispatchEvent(click)
    })
    expect(click.defaultPrevented).toBe(false)
    expect(root.querySelector("[data-testid='where']")!.textContent).toBe("/")
  })

  it("does not open the card a drag ends on", async () => {
    const { frame, layer, root } = await mounted()
    pointer(frame, "pointerdown", 100, 100)
    pointer(frame, "pointermove", 140, 120)
    pointer(frame, "pointerup", 140, 120)
    expect(translateOf(layer)).toEqual([240, 220])
    act(() => {
      frame
        .querySelector("a[href^='/'] text")!
        .dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }))
    })
    expect(root.querySelector("[data-testid='where']")!.textContent).toBe("/")
  })

  it("takes a press that barely moves as a click", async () => {
    const { frame, layer } = await mounted()
    pointer(frame, "pointerdown", 100, 100)
    pointer(frame, "pointermove", 101, 101)
    pointer(frame, "pointerup", 101, 101)
    expect(translateOf(layer)).toEqual([200, 200])
  })
})
