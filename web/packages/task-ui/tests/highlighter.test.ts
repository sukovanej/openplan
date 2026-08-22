import { describe, expect, it } from "vitest"

import { ensureHighlighter, highlightToHast, resolveLang } from "../src/highlighter"

describe("resolveLang", () => {
  it("maps a fence tag to a curated grammar", () => {
    expect(resolveLang("ts")).toBe("typescript")
    expect(resolveLang("RS")).toBe("rust")
    expect(resolveLang(" bash ")).toBe("shellscript")
    expect(resolveLang("patch")).toBe("diff")
  })

  it("refuses a tag outside the curated set", () => {
    expect(resolveLang("brainfuck")).toBeNull()
    expect(resolveLang("")).toBeNull()
    expect(resolveLang(undefined)).toBeNull()
  })
})

describe("the highlighter", () => {
  it("builds once", async () => {
    expect(ensureHighlighter()).toBe(ensureHighlighter())
    await ensureHighlighter()
  })

  it("colours a token with both palettes once it is built", async () => {
    await ensureHighlighter()
    const tree = highlightToHast("const a = 1", "typescript")!
    const pre = tree.children[0]
    expect(pre.type === "element" && pre.tagName).toBe("pre")
    expect(JSON.stringify(tree)).toContain("--shiki-dark")
    expect(JSON.stringify(tree)).toContain("--shiki-light")
  })
})
