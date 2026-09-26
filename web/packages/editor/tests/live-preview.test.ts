import { markdown, markdownLanguage } from "@codemirror/lang-markdown"
import { EditorSelection, EditorState } from "@codemirror/state"
import type { DecorationSet } from "@codemirror/view"
import { describe, expect, it } from "vitest"

import { previewOf } from "../src/live-preview"
import { CheckboxWidget, ConflictWidget, DiagramWidget, MarkdownBlockWidget, TaskRefWidget } from "../src/widgets"

interface Found {
  readonly from: number
  readonly to: number
  readonly widget: unknown
  readonly className: string | undefined
  readonly block: boolean
}

function preview(doc: string, caret?: number) {
  const state = EditorState.create({
    doc,
    selection: caret === undefined ? undefined : EditorSelection.cursor(caret),
    extensions: [markdown({ base: markdownLanguage })],
  })
  return previewOf(state, caret !== undefined, "DEM")
}

function list(set: DecorationSet): Found[] {
  const found: Found[] = []
  set.between(0, Number.MAX_SAFE_INTEGER, (from, to, decoration) => {
    const spec = decoration.spec as { widget?: unknown; class?: string; block?: boolean }
    found.push({ from, to, widget: spec.widget, className: spec.class, block: spec.block === true })
  })
  return found
}

const hidden = (doc: string, caret?: number) =>
  list(preview(doc, caret).decorations)
    .filter((found) => found.from < found.to && found.widget === undefined && found.className === undefined)
    .map(({ from, to }) => doc.slice(from, to))

const widgets = (doc: string, caret?: number) =>
  list(preview(doc, caret).decorations).filter((found) => found.widget !== undefined)

const lineClasses = (doc: string, caret?: number) =>
  list(preview(doc, caret).decorations).flatMap((found) =>
    found.from === found.to && found.className !== undefined ? [found.className] : [],
  )

describe("the live preview", () => {
  it("hides emphasis marks away from the caret", () => {
    expect(hidden("Some *intro* text\n")).toEqual(["*", "*"])
  })

  it("shows the marks of the emphasis the caret is in", () => {
    expect(hidden("Some *intro* text\n", 8)).toEqual([])
  })

  it("keeps other constructs rendered while the caret is on one", () => {
    expect(hidden("*a* and **b**\n", 1)).toEqual(["**", "**"])
  })

  it("hides a heading mark unless the caret is on its line", () => {
    expect(hidden("## Plan\n\ntext\n")).toEqual(["## "])
    expect(hidden("## Plan\n\ntext\n", 5)).toEqual([])
  })

  it("shows a link as its text", () => {
    expect(hidden("see [docs](https://example.com) now\n")).toEqual(["[", "](https://example.com)"])
  })

  it("shows an autolink as its address", () => {
    expect(hidden("see <https://example.com> now\n")).toEqual(["<", ">"])
  })

  it("renders a reference to a task of this store as a chip", () => {
    const [chip] = widgets("See [[DEM-1]].\n")
    expect(chip.widget).toBeInstanceOf(TaskRefWidget)
    expect([chip.from, chip.to]).toEqual([4, 13])
  })

  it("renders a reference to a doc as a chip", () => {
    const [chip] = widgets("See [[storage#Layout]].\n")
    expect(chip.widget).toBeInstanceOf(TaskRefWidget)
    expect([chip.from, chip.to]).toEqual([4, 22])
  })

  it("keeps the source of a reference the caret touches, one in code, and one to another store", () => {
    expect(widgets("See [[DEM-1]].\n", 13)).toEqual([])
    expect(widgets("Use `[[DEM-1]]`.\n")).toEqual([])
    expect(widgets("See [[XYZ-1]].\n")).toEqual([])
  })

  it("renders a checklist marker as a checkbox", () => {
    const [box] = widgets("- [x] done\n")
    expect(box.widget).toBeInstanceOf(CheckboxWidget)
    expect([box.from, box.to]).toEqual([0, 6])
  })

  it("hides the fences of a code block away from the caret", () => {
    const doc = "```rust\nfn main() {}\n```\n"
    expect(lineClasses(doc).filter((name) => name === "cm-fence-hidden")).toHaveLength(2)
    expect(lineClasses(doc, 10).filter((name) => name === "cm-fence-hidden")).toHaveLength(0)
  })

  it("draws a diagram away from the caret and shows its source at it", () => {
    const doc = "```mermaid\ngraph TD\n  A --> B\n```\n"
    expect(widgets(doc)[0]?.widget).toBeInstanceOf(DiagramWidget)
    expect(widgets(doc, 15)).toEqual([])
  })

  it("renders a table away from the caret", () => {
    const doc = "| a | b |\n|---|---|\n| 1 | 2 |\n"
    const [table] = widgets(doc)
    expect(table.widget).toBeInstanceOf(MarkdownBlockWidget)
    expect(table.block).toBe(true)
    expect(widgets(doc, 3)).toEqual([])
  })

  it("renders a conflict block whole, even at the caret, and keeps the caret out of it", () => {
    const doc = "Before.\n\n<<<<<<< ours\nmine\n=======\ntheirs\n>>>>>>> theirs\n\nAfter.\n"
    const start = doc.indexOf("<<<")
    const [conflict] = widgets(doc, start + 2)
    expect(conflict.widget).toBeInstanceOf(ConflictWidget)
    expect(list(preview(doc, start + 2).atomic)).toHaveLength(1)
  })
})
