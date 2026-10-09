import { describe, expect, it } from "vitest"

import { MenuTrigger } from "../src/menu-trigger"
import { render } from "./render"

describe("MenuTrigger", () => {
  it("says whether its menu is open", () => {
    const button = (open: boolean) => render(<MenuTrigger open={open}>alpha</MenuTrigger>).querySelector("button")!
    expect(button(true).getAttribute("aria-expanded")).toBe("true")
    expect(button(false).getAttribute("aria-expanded")).toBe("false")
  })

  it("is a plain button, so a click inside a form does not submit it", () => {
    const container = render(<MenuTrigger open={false}>alpha</MenuTrigger>)
    expect(container.querySelector("button")!.getAttribute("type")).toBe("button")
  })
})
