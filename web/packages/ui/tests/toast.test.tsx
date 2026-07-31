import { act } from "react"
import { describe, expect, it, vi } from "vitest"

import { Toast } from "../src/toast"
import { render } from "./render"

const surfaceOf = (container: HTMLElement) => container.firstElementChild!.firstElementChild as HTMLElement | null

describe("Toast", () => {
  it("keeps the live region mounted while it has nothing to say", () => {
    const container = render(<Toast role="status" live="polite" />)
    const region = container.firstElementChild!
    expect(region.getAttribute("role")).toBe("status")
    expect(region.getAttribute("aria-live")).toBe("polite")
    expect(surfaceOf(container)).toBeNull()
  })

  it("draws the ok tone on the plain surface", () => {
    const classes = surfaceOf(render(<Toast role="status">Copied</Toast>))!.className
    expect(classes).toContain("bg-background")
    expect(classes).not.toContain("bg-danger-surface")
  })

  it("draws the danger tone from the tokens", () => {
    const classes = surfaceOf(
      render(
        <Toast role="alert" tone="danger">
          Nope
        </Toast>,
      ),
    )!.className
    expect(classes).toContain("bg-danger-surface")
    expect(classes).toContain("text-danger-foreground")
    expect(classes).toContain("border-danger-border/30")
  })

  it("takes the shape the caller asks for", () => {
    expect(surfaceOf(render(<Toast role="status">hi</Toast>))!.className).toContain("rounded-full")
    expect(
      surfaceOf(
        render(
          <Toast role="alert" shape="card">
            hi
          </Toast>,
        ),
      )!.className,
    ).toContain("rounded-lg")
  })

  it("shows a dismiss only when there is a handler, and fires it", () => {
    expect(render(<Toast role="alert">hi</Toast>).querySelector('[aria-label="Dismiss"]')).toBeNull()

    const onDismiss = vi.fn()
    const container = render(
      <Toast role="alert" tone="danger" shape="card" onDismiss={onDismiss}>
        Something failed
      </Toast>,
    )
    act(() => {
      container.querySelector<HTMLElement>('[aria-label="Dismiss"]')!.click()
    })
    expect(onDismiss).toHaveBeenCalledOnce()
  })
})
