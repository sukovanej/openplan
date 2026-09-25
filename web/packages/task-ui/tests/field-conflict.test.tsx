import { act } from "react"
import { describe, expect, it, vi } from "vitest"

import { type ConflictChoice, FieldConflict } from "../src/field-conflict"
import { render } from "./render"

const buttons = (container: HTMLElement) => [...container.querySelectorAll("button")]
const named = (container: HTMLElement, text: string) => buttons(container).find((each) => each.textContent === text)

const open = (container: HTMLElement) => act(() => container.querySelector("button")!.click())

describe("FieldConflict", () => {
  it("shows a conflict mark and keeps the versions closed until asked", () => {
    const container = render(<FieldConflict field="Status" choices={[]} />)
    const trigger = container.querySelector("button")!
    expect(trigger.textContent).toBe("Conflict")
    expect(trigger.getAttribute("aria-label")).toBe("Status: two versions from a sync")
    expect(container.querySelector("[role=dialog]")).toBeNull()
  })

  it("lists each version with its label and value, the last one in force", () => {
    const container = render(
      <FieldConflict
        field="Status"
        choices={[
          { label: "Ann (a1b2c3d)", value: "In progress" },
          { label: "Ben (e4f5a6b)", value: "Done" },
        ]}
      />,
    )
    open(container)
    const versions = [...container.querySelectorAll("[role=dialog] li")]
    expect(versions.map((each) => each.textContent)).toEqual(["Ann (a1b2c3d)In progress", "Ben (e4f5a6b)in forceDone"])
  })

  it("keeps the version picked, and closes", () => {
    const kept: Array<string> = []
    const choices: ReadonlyArray<ConflictChoice> = [
      { label: "Ann", value: "a", keep: () => kept.push("Ann") },
      { label: "Ben", value: "b", keep: () => kept.push("Ben") },
    ]
    const container = render(<FieldConflict field="Rank" choices={choices} />)
    open(container)
    act(() => named(container, "Keep Ann")!.click())
    expect(kept).toEqual(["Ann"])
    expect(container.querySelector("[role=dialog]")).toBeNull()
  })

  it("offers no button for a version that cannot be written back", () => {
    const container = render(
      <FieldConflict
        field="Created"
        choices={[
          { label: "Ann", value: "a" },
          { label: "Ben", value: "b" },
        ]}
      />,
    )
    open(container)
    expect(buttons(container).map((each) => each.textContent)).toEqual(["Conflict"])
  })

  it("holds its buttons while a write is on its way", () => {
    const keep = vi.fn()
    const container = render(
      <FieldConflict field="Status" pending choices={[{ label: "Ann", value: "a", keep }]} trigger="Status" />,
    )
    open(container)
    expect(named(container, "Keep Ann")?.disabled).toBe(true)
  })
})
