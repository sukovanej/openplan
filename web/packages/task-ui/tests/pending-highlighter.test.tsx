import { act } from "react"
import { expect, it } from "vitest"

import { ensureHighlighter, highlightToHast } from "../src/highlighter"
import { TaskBody } from "../src/task-body"
import { render } from "./render"

// The highlighter is a module singleton, so the state before it is built survives in this file
// alone, and in one test — anything that builds it first would take the state away.
it("renders a fence plain until the highlighter is built", async () => {
  expect(highlightToHast("const a = 1", "typescript")).toBeNull()

  const markdown = "```ts\nconst a = 1\n```"
  const root = render(<TaskBody project="open-plan" abbreviation="OPP" markdown={markdown} />)
  const plain = root.querySelector("pre code")!
  expect(root.querySelector(".shiki")).toBeNull()
  expect(plain.className).toBe("language-ts")

  await act(async () => {
    await ensureHighlighter()
  })
  expect(root.querySelector("pre.shiki")).not.toBeNull()
  expect(root.querySelector("pre")!.textContent).toBe(plain.textContent)
})
