import { afterEach, describe, expect, it } from "vitest"

import { historyIndex } from "../../src/lib/keys/history"

const setHistory = (state: unknown) => {
  Object.defineProperty(globalThis, "history", { value: { state }, configurable: true })
}

afterEach(() => {
  Reflect.deleteProperty(globalThis, "history")
})

describe("historyIndex", () => {
  it("reads the router's entry index", () => {
    setHistory({ idx: 3, usr: null, key: "abc" })
    expect(historyIndex()).toBe(3)
  })

  it("falls back to the first entry when the router has not stamped one", () => {
    expect(historyIndex()).toBe(0)
    setHistory(null)
    expect(historyIndex()).toBe(0)
    setHistory({ usr: null })
    expect(historyIndex()).toBe(0)
    setHistory({ idx: "2" })
    expect(historyIndex()).toBe(0)
  })

  it("distinguishes a forward from a back, which navigation types cannot", () => {
    // Both report POP, so a counter unwound on POP would drop to 0 on a Forward and send Esc to
    // the list; comparing indices against the entry we arrived on keeps Back available.
    const entry = 1
    setHistory({ idx: 2 })
    expect(historyIndex() > entry).toBe(true)
    setHistory({ idx: 1 })
    expect(historyIndex() > entry).toBe(false)
    setHistory({ idx: 3 })
    expect(historyIndex() > entry).toBe(true)
  })
})
