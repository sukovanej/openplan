import { describe, expect, it } from "vitest"

import { Row } from "../src/row"
import { render } from "./render"

const classesOf = (node: HTMLElement) => (node.firstElementChild as HTMLElement).className.split(" ")

describe("Row", () => {
  it("draws the outline as an overlay, so selecting a row cannot change its height", () => {
    const classes = classesOf(render(<Row active />))
    expect(classes).toContain("after:absolute")
    expect(classes).toContain("after:-top-px")
    expect(classes).toContain("after:-bottom-px")
    expect(classes).toContain("border-transparent")
  })

  it("repeats the treatment under the pointer when hoverable", () => {
    const classes = classesOf(render(<Row hoverable />))
    expect(classes).toContain("hover:after:absolute")
    expect(classes).toContain("hover:bg-muted/30")
    expect(classes).not.toContain("after:absolute")
  })

  it("leaves the current row to its own treatment rather than layering hover over it", () => {
    const classes = classesOf(render(<Row active hoverable />))
    expect(classes).toContain("after:absolute")
    expect(classes).not.toContain("hover:after:absolute")
  })

  it("drops the divider on the last row", () => {
    expect(classesOf(render(<Row last />))).toContain("border-transparent")
    expect(classesOf(render(<Row />))).not.toContain("border-transparent")
  })

  it("fills the current option instead of outlining it", () => {
    const classes = classesOf(render(<Row variant="option" active />))
    expect(classes).toContain("bg-accent")
    expect(classes).not.toContain("after:absolute")
  })

  it("renders the element the caller asks for, with its props", () => {
    const row = render(<Row as="li" variant="option" role="option" aria-selected />).firstElementChild!
    expect(row.tagName).toBe("LI")
    expect(row.getAttribute("aria-selected")).toBe("true")
  })
})
