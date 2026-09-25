import { describe, expect, it } from "vitest"

import { clampFrame, MIN_SIZE, moveFrame, parseFrame, resizeFrame } from "../src/lib/floating-frame"

const viewport = { width: 1200, height: 800 }
const frame = { x: 100, y: 300, width: 600, height: 400 }

describe("moveFrame", () => {
  it("moves by the drag", () => {
    expect(moveFrame(frame, 50, -100, viewport)).toEqual({ ...frame, x: 150, y: 200 })
  })

  it("stops at the window's edges", () => {
    expect(moveFrame(frame, -500, 500, viewport)).toEqual({ ...frame, x: 0, y: 400 })
  })
})

describe("resizeFrame", () => {
  it("grows the right and bottom edges, and keeps the top-left corner", () => {
    expect(resizeFrame(frame, { right: true, bottom: true }, 100, 50, viewport)).toEqual({
      x: 100,
      y: 300,
      width: 700,
      height: 450,
    })
  })

  it("moves the left and top edges, and keeps the opposite ones", () => {
    expect(resizeFrame(frame, { left: true, top: true }, -50, -100, viewport)).toEqual({
      x: 50,
      y: 200,
      width: 650,
      height: 500,
    })
  })

  it("stops at the least size rather than sliding", () => {
    expect(resizeFrame(frame, { left: true }, 1000, 0, viewport)).toEqual({
      ...frame,
      x: 700 - MIN_SIZE.width,
      width: MIN_SIZE.width,
    })
    expect(resizeFrame(frame, { bottom: true }, 0, -1000, viewport)).toEqual({ ...frame, height: MIN_SIZE.height })
  })

  it("stops at the window's edges", () => {
    expect(resizeFrame(frame, { right: true }, 1000, 0, viewport)).toEqual({ ...frame, width: 1100 })
    expect(resizeFrame(frame, { top: true }, 0, -1000, viewport)).toEqual({ ...frame, y: 0, height: 700 })
  })
})

describe("clampFrame", () => {
  it("brings a frame kept from a larger window back on screen", () => {
    expect(clampFrame({ x: 1500, y: 900, width: 2000, height: 400 }, viewport)).toEqual({
      x: 0,
      y: 400,
      width: 1200,
      height: 400,
    })
  })
})

describe("parseFrame", () => {
  it("reads a stored frame and refuses anything else", () => {
    expect(parseFrame(JSON.stringify(frame))).toEqual(frame)
    expect(parseFrame(null)).toBeUndefined()
    expect(parseFrame("not json")).toBeUndefined()
    expect(parseFrame(JSON.stringify({ x: 1, y: 2, width: "wide", height: 3 }))).toBeUndefined()
  })
})
