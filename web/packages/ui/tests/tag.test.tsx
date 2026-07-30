import { describe, expect, it } from "vitest"

import { Tag } from "../src/tag"
import { render } from "./render"

describe("Tag", () => {
  it("renders as static text when it cannot be selected", () => {
    const tag = render(<Tag title="a branch">main</Tag>).firstElementChild!
    expect(tag.tagName).toBe("SPAN")
    expect(tag.textContent).toBe("main")
    expect(tag.getAttribute("title")).toBe("a branch")
  })

  it("becomes a toggle button once given a handler", () => {
    let selected = 0
    const tag = render(
      <Tag selected onSelect={() => selected++}>
        main
      </Tag>,
    ).firstElementChild as HTMLButtonElement
    expect(tag.tagName).toBe("BUTTON")
    expect(tag.getAttribute("aria-pressed")).toBe("true")
    tag.click()
    expect(selected).toBe(1)
  })

  it("takes its hue from the caller and marks itself dashed on request", () => {
    const tag = render(
      <Tag className="border-danger text-danger" dashed>
        gone
      </Tag>,
    ).firstElementChild!
    expect(tag.className).toContain("border-danger")
    expect(tag.className).toContain("text-danger")
    expect(tag.className).toContain("border-dashed")
  })
})
