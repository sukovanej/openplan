import { describe, expect, it } from "vitest"

import { escapeOutcome } from "../src/lib/detail-actions"

describe("escapeOutcome", () => {
  it("clears a live subtask selection before it navigates anywhere", () => {
    expect(escapeOutcome(true, true)).toBe("clear-selection")
    expect(escapeOutcome(true, false)).toBe("clear-selection")
  })

  it("pops in-app history when there is a selection-free entry to return to", () => {
    expect(escapeOutcome(false, true)).toBe("back")
  })

  it("falls back to the list when nothing of ours is left to pop", () => {
    expect(escapeOutcome(false, false)).toBe("to-list")
  })
})
