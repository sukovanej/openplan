import { describe, expect, it } from "vitest"

import { BoundedCache } from "../src/bounded-cache"

describe("BoundedCache", () => {
  it("makes a value once, and gives the same value back after", () => {
    const cache = new BoundedCache<string, object>(2)
    let made = 0
    const make = () => ({ made: ++made })
    expect(cache.remember("a", make)).toBe(cache.remember("a", make))
    expect(made).toBe(1)
  })

  it("keeps no more values than its limit, and drops the one used longest ago", () => {
    const cache = new BoundedCache<string, string>(2)
    cache.remember("a", () => "A")
    cache.remember("b", () => "B")
    cache.remember("a", () => "not made")
    cache.remember("c", () => "C")

    expect(cache.size).toBe(2)
    expect(cache.has("a")).toBe(true)
    expect(cache.has("b")).toBe(false)
    expect(cache.has("c")).toBe(true)
  })
})
