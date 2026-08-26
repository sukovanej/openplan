import { act } from "react"
import { describe, expect, it } from "vitest"

import type { Color } from "@open-planner/api-client"

import { ColorPicker, TAG_COLORS } from "../src/tag-palette"
import { render } from "./render"

const swatches = (container: HTMLElement) => [...container.querySelectorAll("button")]

describe("ColorPicker", () => {
  // The palette is closed and the swatch classes are spelled out one by one, because Tailwind emits
  // only the class names it can read literally — so every colour owes the picker a swatch of its own.
  it("offers every colour in the palette, each in its own fill", () => {
    const buttons = swatches(render(<ColorPicker value="blue" onPick={() => {}} />))
    expect(buttons.map((button) => button.getAttribute("aria-label"))).toEqual([
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
    ])
    expect(TAG_COLORS).toEqual(buttons.map((button) => button.getAttribute("aria-label")))
    for (const [index, color] of TAG_COLORS.entries()) {
      expect(buttons[index].className).toContain(`bg-tag-${color}`)
    }
  })

  it("marks the colour the tag carries, and only that one", () => {
    const buttons = swatches(render(<ColorPicker value="teal" onPick={() => {}} />))
    const checked = buttons.filter((button) => button.getAttribute("aria-checked") === "true")
    expect(checked.map((button) => button.getAttribute("aria-label"))).toEqual(["teal"])
  })

  it("reports the colour that was picked", () => {
    const picked: Array<Color> = []
    const buttons = swatches(render(<ColorPicker value="blue" onPick={(color) => picked.push(color)} />))
    act(() => buttons[1].click())
    expect(picked).toEqual(["red"])
  })
})
