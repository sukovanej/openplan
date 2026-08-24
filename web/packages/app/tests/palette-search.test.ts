import { describe, expect, it } from "vitest"

import type { SearchHit } from "@open-planner/api-client"

import { hitKey, hitPath } from "../src/lib/palette-search"

const hit = (over: Partial<SearchHit["task"]> & { branch: string }): SearchHit => ({
  branch: over.branch,
  task: {
    project: "open-plan",
    id: "OPP-31",
    title: "Palette",
    metadata: { status: "todo", created: "2026-01-01T00:00:00Z", parent: null, rank: null, dependencies: [], tags: [] },
    updated: "2026-01-01T00:00:00Z",
    headline: "main",
    branches: [],
    ...over,
  },
})

describe("where a search hit leads", () => {
  it("leaves the branch off when the hit is on the headline branch", () => {
    expect(hitPath(hit({ branch: "main" }))).toBe("/open-plan/task/OPP-31")
  })

  it("pins the branch the query matched when it is not the headline", () => {
    expect(hitPath(hit({ branch: "feature/a b" }))).toBe("/open-plan/task/OPP-31?branch=feature%2Fa%20b")
  })

  it("names a row by project, key, and branch, which no two rows share", () => {
    expect(hitKey(hit({ branch: "dev" }))).toBe("open-plan OPP-31 dev")
  })
})
