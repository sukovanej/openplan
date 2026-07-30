import { describe, expect, it } from "vitest"

import { relativeTime } from "../src/time"

const NOW = Date.parse("2026-07-28T12:00:00Z")
const ago = (seconds: number) => new Date(NOW - seconds * 1000).toISOString()

describe("relativeTime", () => {
  it("reports a span in the coarsest unit it fills", () => {
    expect(relativeTime(ago(30), NOW)).toBe("just now")
    expect(relativeTime(ago(90), NOW)).toBe("1 minute ago")
    expect(relativeTime(ago(2 * 60 * 60), NOW)).toBe("2 hours ago")
    expect(relativeTime(ago(3 * 24 * 60 * 60), NOW)).toBe("3 days ago")
    expect(relativeTime(ago(14 * 24 * 60 * 60), NOW)).toBe("2 weeks ago")
  })

  // Rounding the count after the unit was chosen prints the next unit's value under this unit's
  // name: 23h50m as "24 hours ago", 59m40s as "60 minutes ago".
  it("never overflows its own unit", () => {
    expect(relativeTime(ago(23 * 60 * 60 + 50 * 60), NOW)).toBe("23 hours ago")
    expect(relativeTime(ago(59 * 60 + 40), NOW)).toBe("59 minutes ago")
    expect(relativeTime(ago(6 * 24 * 60 * 60 + 20 * 60 * 60), NOW)).toBe("6 days ago")
  })

  it("treats a span the same in either direction", () => {
    expect(relativeTime(ago(45 * 24 * 60 * 60), NOW)).toBe("1 month ago")
    expect(relativeTime(ago(-45 * 24 * 60 * 60), NOW)).toBe("in 1 month")
  })

  it("passes an unparseable value through rather than rendering NaN", () => {
    expect(relativeTime("not a date", NOW)).toBe("not a date")
  })
})
