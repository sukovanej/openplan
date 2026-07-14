import { describe, expect, it } from "vitest"

import { chordOf, normalizeToken, sameChord, startsWith } from "../../src/lib/keys/match"

describe("normalizeToken", () => {
  it("lowercases single characters and preserves named keys", () => {
    expect(normalizeToken("j")).toBe("j")
    expect(normalizeToken("Enter")).toBe("Enter")
    expect(normalizeToken("Escape")).toBe("Escape")
    expect(normalizeToken("?")).toBe("?")
  })

  it("normalizes Cmd, Ctrl, and Mod to a single modifier so a palette binding is a drop-in", () => {
    expect(normalizeToken("Cmd+K")).toBe("mod+k")
    expect(normalizeToken("Ctrl+k")).toBe("mod+k")
    expect(normalizeToken("Mod+K")).toBe("mod+k")
    expect(normalizeToken("Meta+k")).toBe("mod+k")
  })

  it("keeps Shift only where the glyph cannot encode it, so Mod+Shift+K stays distinct from Mod+K", () => {
    expect(normalizeToken("Mod+Shift+K")).toBe("mod+shift+k")
    expect(normalizeToken("Mod+K")).toBe("mod+k")
    expect(normalizeToken("Shift+Tab")).toBe("shift+Tab")
    // A bare printable key already carries Shift in its glyph, so it is not recorded again.
    expect(normalizeToken("?")).toBe("?")
  })
})

describe("chordOf", () => {
  it("wraps a single key and normalizes a sequence", () => {
    expect(chordOf("j")).toEqual(["j"])
    expect(chordOf(["g", "l"])).toEqual(["g", "l"])
  })
})

describe("sequence helpers", () => {
  it("startsWith detects a prefix", () => {
    expect(startsWith(["g", "l"], ["g"])).toBe(true)
    expect(startsWith(["g", "l"], ["g", "l"])).toBe(true)
    expect(startsWith(["g", "l"], ["l"])).toBe(false)
    expect(startsWith(["g"], ["g", "l"])).toBe(false)
  })

  it("sameChord requires equal length and order", () => {
    expect(sameChord(["g", "l"], ["g", "l"])).toBe(true)
    expect(sameChord(["g"], ["g", "l"])).toBe(false)
  })
})
