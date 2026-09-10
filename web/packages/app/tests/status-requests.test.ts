import { describe, expect, it } from "vitest"

import { NO_ROW, sameTarget, statusRequests, type StatusTarget } from "../src/lib/status-requests"

const target = (id: string, at: number): StatusTarget => ({ project: "openplan", id, at })

function listen(): { readonly opened: Array<StatusTarget>; readonly stop: () => void } {
  const opened: Array<StatusTarget> = []
  const stop = statusRequests.on((asked) => opened.push(asked))
  return { opened, stop }
}

describe("status requests", () => {
  it("reaches every listener", () => {
    const first = listen()
    const second = listen()
    statusRequests.emit(target("12", 0))
    expect(first.opened).toEqual([target("12", 0)])
    expect(second.opened).toEqual([target("12", 0)])
    first.stop()
    second.stop()
  })

  it("stops reaching a listener that unsubscribed", () => {
    const { opened, stop } = listen()
    stop()
    statusRequests.emit(target("12", 0))
    expect(opened).toEqual([])
  })
})

describe("which control a request names", () => {
  it("matches the same task on the same row", () => {
    expect(sameTarget(target("12", 0), target("12", 0))).toBe(true)
  })

  // The task detail lists one task under both "Blocks" and "Subtasks", and the page's own task wears
  // a mark of its own on no row at all.
  it("tells two rows of one task apart, and both from the page's own mark", () => {
    expect(sameTarget(target("12", 0), target("12", 1))).toBe(false)
    expect(sameTarget(target("12", NO_ROW), target("12", 0))).toBe(false)
  })

  it("tells the same key in two projects apart", () => {
    expect(sameTarget({ ...target("12", 0), project: "other" }, target("12", 0))).toBe(false)
  })
})
