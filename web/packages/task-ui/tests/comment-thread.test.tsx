import { act } from "react"
import { describe, expect, it } from "vitest"

import type { Comment } from "@open-planner/api-client"
import { absoluteTime } from "@open-planner/ui"

import { CommentThread } from "../src/comment-thread"
import { render } from "./render"

const comment = (over: Partial<Comment> = {}): Comment => ({
  at: "2026-08-24T09:12:04Z",
  author: "Milan Suk",
  agent: null,
  text: "hello",
  ...over,
})

const thread = (comments: ReadonlyArray<Comment>) =>
  render(<CommentThread project="open-plan" comments={comments} abbreviation="OPP" />)

describe("CommentThread", () => {
  it("shows every entry with its author, absolute time, and agent", () => {
    const root = thread([comment({ agent: "claude-code" }), comment({ author: "Ada", text: "second" })])

    const entries = root.querySelectorAll("li")
    expect(entries).toHaveLength(2)
    expect(entries[0].textContent).toContain("Milan Suk")
    expect(entries[0].textContent).toContain("claude-code")
    expect(entries[0].querySelector("time")?.getAttribute("datetime")).toBe("2026-08-24T09:12:04Z")
    expect(entries[0].querySelector("time")?.textContent).toBe(absoluteTime("2026-08-24T09:12:04Z"))
    expect(entries[1].textContent).toContain("Ada")
    expect(entries[1].textContent).not.toContain("claude-code")
  })

  it("renders the comment text as markdown", () => {
    const root = thread([comment({ text: "# A quoted heading\n\n- a list item" })])

    expect(root.querySelector("h1")?.textContent).toBe("A quoted heading")
    expect(root.querySelector("li li")?.textContent).toBe("a list item")
  })

  it("keeps a damaged entry's text and names the field that failed", () => {
    const root = thread([
      comment({
        at: { kind: "invalid", message: 'not an RFC3339 UTC timestamp: "yesterday"' },
        text: "still readable",
      }),
    ])

    expect(root.textContent).toContain("still readable")
    expect(root.textContent).toContain('not an RFC3339 UTC timestamp: "yesterday"')
    expect(root.querySelector("time")).toBe(null)
  })

  it("names a missing author rather than leaving the entry unsigned", () => {
    const root = thread([comment({ author: { kind: "missing" }, text: "orphan" })])

    expect(root.textContent).toContain("missing")
    expect(root.textContent).toContain("orphan")
  })

  it("says so when the log is empty", () => {
    const root = thread([])

    expect(root.textContent).toContain("No comments yet.")
    expect(root.querySelectorAll("li")).toHaveLength(0)
  })

  it("explains a damaged field when the keyboard lands on it", () => {
    const root = thread([comment({ author: { kind: "missing" } })])

    act(() => root.querySelector<HTMLElement>("[tabindex]")?.focus())
    expect(root.querySelector("[role=tooltip]")?.textContent).toBe("missing")
  })
})
