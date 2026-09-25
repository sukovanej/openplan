import { act } from "react"
import { describe, expect, it } from "vitest"

import { BodyConflict, TaskBodyWithConflicts } from "../src/body-conflict"
import { bodySegments, type ConflictBlock } from "../src/body-segments"
import { render } from "./render"

const BLOCK = "<<<<<<< Ann (a1b2c3d)\nUse **OAuth** only.\n=======\nUse OAuth and email login.\n>>>>>>> Ben (e4f5a6b)\n"

const conflict: ConflictBlock = {
  block: BLOCK,
  other: { label: "Ann (a1b2c3d)", text: "Use **OAuth** only.\n" },
  published: { label: "Ben (e4f5a6b)", text: "Use OAuth and email login.\n" },
}

const named = (container: HTMLElement, text: string) =>
  [...container.querySelectorAll("button")].find((each) => each.textContent === text)

const click = (container: HTMLElement, text: string) => act(() => named(container, text)!.click())

// React tracks the last value it wrote to the node, so assigning `value` directly leaves it believing
// nothing changed. The prototype setter is what its tracker watches.
const setValue = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, "value")!.set!

const type = (textarea: HTMLTextAreaElement, text: string) =>
  act(() => {
    setValue.call(textarea, text)
    textarea.dispatchEvent(new Event("input", { bubbles: true }))
  })

function resolved(onResolve: (text: string) => void) {
  return render(<BodyConflict project="openplan" abbreviation="OPP" conflict={conflict} onResolve={onResolve} />)
}

describe("BodyConflict", () => {
  it("renders each version as markdown under its label, the published one in force", () => {
    const container = render(<BodyConflict project="openplan" abbreviation="OPP" conflict={conflict} />)
    const versions = [...container.querySelectorAll("[role=group]")]
    expect(versions.map((each) => each.getAttribute("aria-label"))).toEqual(["Ann (a1b2c3d)", "Ben (e4f5a6b)"])
    expect(versions[0].querySelector("strong")?.textContent).toBe("OAuth")
    expect(versions[0].textContent).not.toContain("in force")
    expect(versions[1].textContent).toContain("in force")
  })

  it("offers no button when nothing can resolve the block", () => {
    const container = render(<BodyConflict project="openplan" abbreviation="OPP" conflict={conflict} />)
    expect(container.querySelectorAll("button")).toHaveLength(0)
  })

  it.each([
    ["Keep Ann (a1b2c3d)", "Use **OAuth** only.\n"],
    ["Keep Ben (e4f5a6b)", "Use OAuth and email login.\n"],
    ["Keep both", "Use **OAuth** only.\n\nUse OAuth and email login.\n"],
  ])("sends the text %s keeps", (button, text) => {
    const sent: Array<string> = []
    const container = resolved((each) => sent.push(each))
    click(container, button)
    expect(sent).toEqual([text])
  })

  it("edits both versions without their markers, and sends the edited text", () => {
    const sent: Array<string> = []
    const container = resolved((each) => sent.push(each))
    click(container, "Edit")
    const textarea = container.querySelector("textarea")!
    expect(textarea.value).toBe("Use **OAuth** only.\n\nUse OAuth and email login.\n")
    type(textarea, "Use OAuth, and email login later.\n")
    click(container, "Save")
    expect(sent).toEqual(["Use OAuth, and email login later.\n"])
  })

  it("goes back to the two versions on Cancel, and sends nothing", () => {
    const sent: Array<string> = []
    const container = resolved((each) => sent.push(each))
    click(container, "Edit")
    click(container, "Cancel")
    expect(container.querySelector("textarea")).toBeNull()
    expect(container.querySelectorAll("[role=group]")).toHaveLength(2)
    expect(sent).toEqual([])
  })

  it("holds its buttons while a resolve is on its way", () => {
    const container = render(
      <BodyConflict project="openplan" abbreviation="OPP" conflict={conflict} onResolve={() => {}} pending />,
    )
    expect(named(container, "Keep both")?.disabled).toBe(true)
    expect(named(container, "Edit")?.disabled).toBe(true)
  })
})

describe("TaskBodyWithConflicts", () => {
  it("renders the text around a block as markdown, and names the block by its exact text", () => {
    const sent: Array<[string, string]> = []
    const container = render(
      <TaskBodyWithConflicts
        project="openplan"
        abbreviation="OPP"
        segments={bodySegments(`## Login\n\n${BLOCK}\nThe end.\n`)}
        onResolve={(block, text) => sent.push([block, text])}
      />,
    )
    expect(container.querySelector("h2")?.textContent).toBe("Login")
    expect(container.textContent).toContain("The end.")
    click(container, "Keep Ben (e4f5a6b)")
    expect(sent).toEqual([[BLOCK, "Use OAuth and email login.\n"]])
  })

  it("renders a body without blocks as one body", () => {
    const container = render(
      <TaskBodyWithConflicts project="openplan" abbreviation="OPP" segments={bodySegments("Just text.\n")} />,
    )
    expect(container.querySelectorAll("article")).toHaveLength(1)
    expect(container.querySelector("section")).toBeNull()
  })
})
