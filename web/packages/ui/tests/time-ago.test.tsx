import { act } from "react"
import { createRoot } from "react-dom/client"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import { TimeAgo } from "../src/time-ago"
import { render } from "./render"

const NOW = Date.parse("2026-07-28T12:00:00Z")

beforeEach(() => {
  vi.useFakeTimers()
  vi.setSystemTime(NOW)
})

afterEach(() => {
  vi.useRealTimers()
})

describe("TimeAgo", () => {
  it("moves on as time passes, without a new render from its parent", () => {
    const shown = render(<TimeAgo iso={new Date(NOW).toISOString()} label="Synced" />)
    expect(shown.textContent).toBe("just now")

    act(() => vi.advanceTimersByTime(10_000))
    expect(shown.textContent).toBe("10 seconds ago")

    act(() => vi.advanceTimersByTime(50_000))
    expect(shown.textContent).toBe("1 minute ago")
  })

  it("stops its clock once nothing shows a time", () => {
    const root = createRoot(document.createElement("div"))
    act(() => root.render(<TimeAgo iso={new Date(NOW).toISOString()} label="Synced" />))
    expect(vi.getTimerCount()).toBe(1)

    act(() => root.unmount())
    expect(vi.getTimerCount()).toBe(0)
  })
})
