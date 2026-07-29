// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import { copyTaskId, writeClipboard } from "../src/lib/clipboard"
import { flash } from "../src/lib/flash"

function stubClipboard(clipboard: unknown): void {
  Object.defineProperty(navigator, "clipboard", { value: clipboard, configurable: true })
}

beforeEach(() => {
  flash.clear()
})

afterEach(() => {
  vi.useRealTimers()
})

describe("copyTaskId", () => {
  it("writes the id and confirms it", async () => {
    const writeText = vi.fn(() => Promise.resolve())
    stubClipboard({ writeText })

    copyTaskId("28")
    await vi.waitFor(() => expect(flash.getSnapshot()).toEqual({ text: "Copied 28", tone: "ok" }))
    expect(writeText).toHaveBeenCalledWith("28")
  })

  it("reports a rejected write instead of failing silently", async () => {
    stubClipboard({ writeText: () => Promise.reject(new Error("denied")) })

    copyTaskId("28")
    await vi.waitFor(() => expect(flash.getSnapshot()).toEqual({ text: "Copy failed", tone: "error" }))
  })

  it("reports failure where the page has no clipboard at all", async () => {
    stubClipboard(undefined)

    await expect(writeClipboard("28")).rejects.toThrow()
    copyTaskId("28")
    await vi.waitFor(() => expect(flash.getSnapshot()?.tone).toBe("error"))
  })
})

describe("flash", () => {
  it("clears itself once shown", () => {
    vi.useFakeTimers()
    flash.show("Copied 28", "ok")
    expect(flash.getSnapshot()).toEqual({ text: "Copied 28", tone: "ok" })

    vi.advanceTimersByTime(1600)
    expect(flash.getSnapshot()).toBeUndefined()
  })

  it("notifies subscribers on every show, so a repeat copy re-raises the pill", () => {
    let notified = 0
    const unsubscribe = flash.subscribe(() => void notified++)
    flash.show("Copied 28", "ok")
    flash.show("Copied 28", "ok")
    expect(notified).toBe(2)
    unsubscribe()
  })
})
