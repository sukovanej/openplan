import { describe, expect, it } from "vitest"

import { Tag } from "../src/tag"
import { render } from "./render"

describe("Tag", () => {
  it("renders as static text", () => {
    const tag = render(<Tag>backend</Tag>).firstElementChild!
    expect(tag.tagName).toBe("SPAN")
    expect(tag.textContent).toBe("backend")
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
