import { act } from "react"
import { describe, expect, it } from "vitest"

import type { Field_Vec_String, Metadata, TagView } from "@openplan/api-client"

import { TaskTags } from "../src/task-tags"
import { render } from "./render"

const metadata = (tags: Field_Vec_String): Metadata => ({
  status: "todo",
  created: "2026-07-31T14:08:22Z",
  parent: null,
  rank: null,
  dependencies: [],
  tags,
})

const registry = (...tags: ReadonlyArray<TagView>) => new Map(tags.map((tag) => [tag.name, tag]))

const backend: TagView = { name: "backend", display: "Backend", color: "blue" }

const names = (container: HTMLElement) => [...container.querySelectorAll("span.rounded-md")].map((c) => c.textContent)

describe("TaskTags", () => {
  it("renders one chip per name, in the order the task holds them", () => {
    const tags = registry(backend, { name: "wip", display: "WIP", color: "amber" })
    expect(names(render(<TaskTags metadata={metadata(["backend", "wip"])} tags={tags} />))).toEqual(["Backend", "WIP"])
  })

  it("falls back to the raw name for a tag this branch's registry does not hold", () => {
    expect(names(render(<TaskTags metadata={metadata(["backend", "infra"])} tags={registry(backend)} />))).toEqual([
      "Backend",
      "infra",
    ])
  })

  // Before the registry lands, a name the branch holds looks exactly like one it does not.
  it("shows nothing until the registry has been read", () => {
    expect(names(render(<TaskTags metadata={metadata(["backend"])} tags={undefined} />))).toEqual([])
  })

  it("shows nothing for a task with no tags, and nothing for a tags field that could not be read", () => {
    expect(names(render(<TaskTags metadata={metadata([])} tags={registry(backend)} />))).toEqual([])
    expect(
      names(render(<TaskTags metadata={metadata({ kind: "invalid", message: "not a list" })} tags={registry()} />)),
    ).toEqual([])
  })

  it("names each chip's remover after the chip", () => {
    const removed: Array<string> = []
    const container = render(
      <TaskTags metadata={metadata(["backend", "infra"])} tags={registry(backend)} onRemove={(n) => removed.push(n)} />,
    )
    const buttons = [...container.querySelectorAll("button")]
    expect(buttons.map((b) => b.getAttribute("aria-label"))).toEqual(["Remove Backend", "Remove infra"])
    act(() => buttons[1].click())
    expect(removed).toEqual(["infra"])
  })

  // A task carrying no tags still needs somewhere to put the control that adds the first one.
  it("keeps the row on screen for a trailing control, with or without chips", () => {
    const add = <button type="button">Add tag</button>
    expect(render(<TaskTags metadata={metadata([])} tags={registry()} trailing={add} />).textContent).toBe("Add tag")
    expect(render(<TaskTags metadata={metadata([])} tags={undefined} trailing={add} />).textContent).toBe("Add tag")
  })
})
