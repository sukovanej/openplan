import { describe, expect, it } from "vitest"

import type { RevisionView } from "@openplan/api-client"

import { RevisionMeta, shortRevision } from "../src/revision-meta"
import { render } from "./render"

const revision = (over: Partial<RevisionView> = {}): RevisionView => ({
  id: "0123456789abcdef",
  parents: [],
  author: "Milan",
  at: "2026-01-02T00:00:00Z",
  message: "Set OPP-1 to done",
  ...over,
})

describe("shortRevision", () => {
  it("keeps as much of the id as a reader compares", () => {
    expect(shortRevision("0123456789abcdef")).toBe("0123456")
    expect(shortRevision("42")).toBe("42")
  })
})

describe("RevisionMeta", () => {
  it("names the author, the time, and the short id", () => {
    const text = render(<RevisionMeta revision={revision()} />).textContent
    expect(text).toContain("Milan")
    expect(text).toContain("0123456")
    expect(text).not.toContain("0123456789")
  })

  it("names the agent that wrote the revision for the author, and nothing when a person wrote it", () => {
    expect(render(<RevisionMeta revision={revision({ agent: "claude_code" })} />).textContent).toContain("claude_code")
    expect(render(<RevisionMeta revision={revision()} />).querySelector("svg")).toBeNull()
  })
})
