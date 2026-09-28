import { markdown, markdownLanguage } from "@codemirror/lang-markdown"
import { EditorState } from "@codemirror/state"
import { describe, expect, it } from "vitest"

import { listTextStarts } from "../src/hanging-indent"

function starts(doc: string): string[] {
  const state = EditorState.create({ doc, extensions: [markdown({ base: markdownLanguage })] })
  return listTextStarts(state, 0, doc.length).map(({ line, text }) => doc.slice(line, text))
}

describe("the hanging indent", () => {
  it("starts the text of a bullet after the marker", () => {
    expect(starts("- one\n- two\n")).toEqual(["- ", "- "])
  })

  it("starts the text of a numbered item after the number", () => {
    expect(starts("1. one\n10. ten\n")).toEqual(["1. ", "10. "])
  })

  it("starts the text of a checklist item after the checkbox", () => {
    expect(starts("- [ ] todo\n- [x] done\n")).toEqual(["- [ ] ", "- [x] "])
  })

  it("starts the text of a nested item after its indent and marker", () => {
    expect(starts("- one\n  - two\n")).toEqual(["- ", "  - "])
  })

  it("starts a continuation line after its indent", () => {
    expect(starts("- one\n  more\n")).toEqual(["- ", "  "])
  })

  it("leaves a paragraph outside a list alone", () => {
    expect(starts("plain text\n")).toEqual([])
  })
})
