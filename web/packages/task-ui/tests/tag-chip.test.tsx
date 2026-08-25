import { act } from "react"
import { afterEach, describe, expect, it, vi } from "vitest"

import type { Color, TagView } from "@open-planner/api-client"

import { TagChip } from "../src/tag-chip"
import { render } from "./render"

afterEach(() => vi.useRealTimers())

const view = (over: Partial<TagView> = {}): TagView => ({
  name: "backend",
  display: "Backend",
  color: "blue",
  ...over,
})

// A chip with something to explain answers through a `Tooltip`, which anchors it in a span of its own.
const chip = (container: HTMLElement) => container.querySelector("span.rounded-md")!

const hover = (container: HTMLElement) => {
  act(() => void container.firstElementChild!.dispatchEvent(new Event("pointerover", { bubbles: true })))
  act(() => vi.advanceTimersByTime(300))
  return container.querySelector("[role=tooltip]")
}

describe("TagChip", () => {
  it("shows the registry's display name in the registry's colour", () => {
    const tag = chip(render(<TagChip name="backend" tag={view()} />))
    expect(tag.textContent).toBe("Backend")
    expect(tag.className).toContain("border-tag-blue")
    expect(tag.className).toContain("text-tag-blue")
  })

  // The chip spends one palette rung on both its border and its text, and Tailwind emits only the
  // class names it can read literally — so every colour owes the map a pair of its own.
  it("carries classes of its own for every colour in the palette", () => {
    const palette: ReadonlyArray<Color> = [
      "slate",
      "red",
      "orange",
      "amber",
      "yellow",
      "green",
      "teal",
      "cyan",
      "blue",
      "indigo",
      "violet",
      "pink",
    ]
    for (const color of palette) {
      const tag = chip(render(<TagChip name="x" tag={view({ color })} />))
      expect(tag.className).toContain(`border-tag-${color}`)
      expect(tag.className).toContain(`text-tag-${color}`)
    }
  })

  it("explains a tag that carries a description", () => {
    vi.useFakeTimers()
    const container = render(<TagChip name="backend" tag={view({ description: "Server-side work" })} />)
    expect(hover(container)?.textContent).toBe("Server-side work")
  })

  it("says nothing more about a tag its name already spells out", () => {
    vi.useFakeTimers()
    expect(hover(render(<TagChip name="backend" tag={view()} />))).toBeNull()
  })

  describe("on a task whose tags can be edited", () => {
    it("offers to take the tag off, named by what the chip shows", () => {
      let removed = 0
      const container = render(<TagChip name="backend" tag={view()} onRemove={() => removed++} />)
      const remove = container.querySelector("button")!
      expect(remove.getAttribute("aria-label")).toBe("Remove Backend")
      act(() => remove.click())
      expect(removed).toBe(1)
    })

    // Dropping the reference is the only thing a dangling chip can do, and the tooltip still says
    // what an edit costs.
    it("names a dangling reference by the raw name it carries", () => {
      const container = render(<TagChip name="infra" tag={undefined} onRemove={() => {}} />)
      expect(container.querySelector("button")!.getAttribute("aria-label")).toBe("Remove infra")
    })

    it("stays inert without a remover", () => {
      expect(render(<TagChip name="backend" tag={view()} />).querySelector("button")).toBeNull()
    })
  })

  describe("a name the registry does not hold", () => {
    it("falls back to the raw name, greyed and dashed rather than coloured", () => {
      const tag = chip(render(<TagChip name="infra" tag={undefined} />))
      expect(tag.textContent).toBe("infra")
      expect(tag.className).toContain("text-muted-foreground")
      expect(tag.className).toContain("border-dashed")
    })

    // The UI drops dangling names from every tags write, so the chip owes the reader that warning
    // before they touch this task's tags.
    it("says where the tag went, and what an edit does to the reference", () => {
      vi.useFakeTimers()
      const text = hover(render(<TagChip name="infra" tag={undefined} />))?.textContent ?? ""
      expect(text).toContain("infra is not a tag on this branch")
      expect(text).toContain("drops it")
    })
  })
})
