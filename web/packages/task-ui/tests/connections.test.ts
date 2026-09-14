import { describe, expect, it } from "vitest"

import { distinguishConnections } from "../src/connections"

const svgOf = (body: string) => `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">${body}</svg>`

// Two connections share one marker, as d2 draws them without sketch mode.
const PLAIN = svgOf(
  '<defs><marker id="mk"><polygon points="0,0 10,6 0,12" fill="#000E3D" class="connection fill-B1" /></marker></defs>' +
    '<g class="a"><path d="M 0 0 L 50 0" class="connection stroke-B1" style="stroke-width:2;" marker-end="url(#mk)" /></g>' +
    '<g class="b"><path d="M 0 20 L 50 20" class="connection stroke-B1" style="stroke-width:2;" marker-end="url(#mk)" /></g>',
)

// Sketch mode draws the arrowhead as paths of its own at the tip of the line.
const SKETCH = svgOf(
  '<g class="c"><path d="M 0 40 C 10 40, 20 40, 50 40" fill="none" class="connection stroke-B1" />' +
    '<path d="M 0 0 L 10 6" class="connection fill-B1" transform="translate(50 40) rotate(0)" /></g>',
)

const UNDIRECTED = svgOf('<g class="d"><path d="M 0 60 L 50 60" class="connection stroke-B1" /></g>')

const parse = (svg: string) => new DOMParser().parseFromString(svg, "image/svg+xml")
const colourOf = (element: Element, property: "stroke" | "fill") =>
  new RegExp(`(?:^|;)${property}:([^;]+)`).exec(element.getAttribute("style") ?? "")?.[1]

describe("distinguishConnections", () => {
  it("gives each connection a colour of its own", () => {
    const doc = parse(distinguishConnections(PLAIN, "light"))
    const [a, b] = [...doc.querySelectorAll("path.connection")].map((path) => colourOf(path, "stroke"))
    expect(a).toBeDefined()
    expect(b).toBeDefined()
    expect(a).not.toBe(b)
  })

  it("paints each arrowhead with its own line", () => {
    const doc = parse(distinguishConnections(PLAIN, "light"))
    for (const path of doc.querySelectorAll("path.connection")) {
      const id = /url\(#(.+)\)/.exec(path.getAttribute("marker-end")!)![1]
      const tip = doc.querySelector(`marker[id="${id}"] polygon`)!
      expect(colourOf(tip, "fill")).toBe(colourOf(path, "stroke"))
    }
  })

  it("puts a dot on the end a connection leaves from", () => {
    const doc = parse(distinguishConnections(PLAIN, "light"))
    const line = doc.querySelector("g.a path")!
    const dot = doc.querySelector("g.a circle")!
    expect([dot.getAttribute("cx"), dot.getAttribute("cy")]).toEqual(["0", "0"])
    expect(colourOf(dot, "fill")).toBe(colourOf(line, "stroke"))
  })

  it("finds the arrowhead of a sketched connection", () => {
    const doc = parse(distinguishConnections(SKETCH, "light"))
    const [line, tip] = [...doc.querySelectorAll("g.c path")]
    expect(colourOf(tip, "fill")).toBe(colourOf(line, "stroke"))
    const dot = doc.querySelector("g.c circle")!
    expect([dot.getAttribute("cx"), dot.getAttribute("cy")]).toEqual(["0", "40"])
  })

  it("leaves a connection with no arrowhead in the theme colour", () => {
    expect(distinguishConnections(UNDIRECTED, "light")).toBe(UNDIRECTED)
  })

  it("takes lighter colours for the dark canvas", () => {
    const first = (theme: "light" | "dark") =>
      colourOf(parse(distinguishConnections(PLAIN, theme)).querySelector("path.connection")!, "stroke")
    expect(first("dark")).not.toBe(first("light"))
  })
})
