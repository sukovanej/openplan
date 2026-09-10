import { describe, expect, it } from "vitest"

import { compileErrorMessage, sizeOf, svgDataUrl } from "../src/diagram"

describe("compileErrorMessage", () => {
  it("lists each problem without the virtual file name", () => {
    const error = new Error(
      JSON.stringify([
        { range: "index,0:0:0-0:4:4", errmsg: "index:1:1: connection missing destination" },
        { range: "index,1:0:0-1:4:4", errmsg: 'index:2:12: unknown shape "nope"' },
      ]),
    )
    expect(compileErrorMessage(error)).toBe('1:1: connection missing destination\n2:12: unknown shape "nope"')
  })

  it("passes any other message through", () => {
    expect(compileErrorMessage(new Error("worker died"))).toBe("worker died")
    expect(compileErrorMessage("plain")).toBe("plain")
  })
})

describe("sizeOf", () => {
  it("reads the outer viewBox", () => {
    expect(sizeOf('<svg viewBox="0 0 87 453"><svg viewBox="-17 -17 87 453"></svg></svg>')).toEqual({
      width: 87,
      height: 453,
    })
  })

  it("gives zero when there is no viewBox", () => {
    expect(sizeOf("<svg></svg>")).toEqual({ width: 0, height: 0 })
  })
})

it("svgDataUrl encodes the markup", () => {
  expect(svgDataUrl("<svg #/>")).toBe("data:image/svg+xml;charset=utf-8,%3Csvg%20%23%2F%3E")
})
