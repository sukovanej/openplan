import { act } from "react"
import { describe, expect, it } from "vitest"

import { ensureHighlighter } from "../src/highlighter"
import { TaskBody } from "../src/task-body"
import { render } from "./render"

async function highlighted(markdown: string): Promise<HTMLElement> {
  const root = render(<TaskBody project="open-plan" abbreviation="OPP" markdown={markdown} />)
  await act(async () => {
    await ensureHighlighter()
  })
  return root
}

describe("fenced code blocks", () => {
  it("colours every token with both palettes", async () => {
    const root = await highlighted("```ts\nconst a = 1\n```")
    const pre = root.querySelector("pre.shiki")!
    const tokens = [...pre.querySelectorAll("span")].filter((span) =>
      span.getAttribute("style")?.includes("--shiki-dark"),
    )
    expect(tokens.length).toBeGreaterThan(1)
    expect(tokens[0]!.getAttribute("style")).toContain("--shiki-light")
    expect(pre.className).toContain("[&_span]:text-[var(--shiki-light)]")
    expect(pre.textContent).toBe("const a = 1\n")
  })

  it("leaves an unknown language alone", async () => {
    const root = await highlighted("```brainfuck\n+[.]\n```")
    expect(root.querySelector(".shiki")).toBeNull()
    expect(root.querySelector("pre code")!.className).toBe("language-brainfuck")
  })

  it("leaves an untagged fence alone", async () => {
    const root = await highlighted("```\nplain\n```")
    expect(root.querySelector(".shiki")).toBeNull()
    expect(root.querySelector("pre code")!.textContent).toBe("plain\n")
  })

  it("leaves an inline code chip alone", async () => {
    const root = await highlighted("Run `openplan list` now.")
    const code = root.querySelector("code")!
    expect(code.closest("pre")).toBeNull()
    expect(code.className).toBe("")
    expect(code.textContent).toBe("openplan list")
  })
})
