import type { DiagramTheme } from "./diagram"

// d2 draws every connection in the theme's one colour, so lines that cross or share a run cannot be
// told apart. Each directed connection gets a colour of its own, and a dot marks the end it leaves.
const PALETTES: Readonly<Record<DiagramTheme, ReadonlyArray<string>>> = {
  light: ["#2563eb", "#dc2626", "#16a34a", "#d97706", "#9333ea", "#0891b2", "#db2777", "#65a30d"],
  dark: ["#60a5fa", "#f87171", "#4ade80", "#fbbf24", "#c084fc", "#22d3ee", "#f472b6", "#a3e635"],
}

const SVG_NS = "http://www.w3.org/2000/svg"
const THEME_COLOUR = /^(stroke|fill)-[A-Z]{1,2}\d$/
const MARKER_URL = /^url\(#(.+)\)$/
const TRANSLATE = /translate\(\s*(-?[\d.]+)[\s,]+(-?[\d.]+)/
const MARKER_ENDS = ["marker-start", "marker-end"] as const
// Sketch mode draws an arrowhead as a path of its own, placed near the end of the line it tips.
const TIP_REACH = 24

type Point = readonly [number, number]
type Line = { path: Element; from: Point; to: Point; arrowAtStart: boolean; arrowAtEnd: boolean }

export function distinguishConnections(svg: string, theme: DiagramTheme): string {
  const doc = new DOMParser().parseFromString(svg, "image/svg+xml")
  if (doc.querySelector("parsererror") !== null) return svg
  const directed = linesOf(doc).filter((line) => line.arrowAtStart || line.arrowAtEnd)
  if (directed.length === 0) return svg
  const palette = PALETTES[theme]
  directed.forEach((line, index) => {
    const colour = palette[index % palette.length]
    const group = line.path.parentElement!
    for (const part of group.querySelectorAll(".connection")) paint(part, colour)
    for (const end of MARKER_ENDS) recolourMarker(doc, line.path, end, `${index}`, colour)
    if (line.arrowAtStart !== line.arrowAtEnd) group.append(dot(doc, line.arrowAtEnd ? line.from : line.to, colour))
  })
  return new XMLSerializer().serializeToString(doc)
}

function linesOf(doc: Document): Array<Line> {
  return [...doc.querySelectorAll("path.connection:not([transform])")]
    .filter((path) => path.closest("marker") === null)
    .map((path) => {
      const numbers =
        path
          .getAttribute("d")
          ?.match(/-?\d+(?:\.\d+)?/g)
          ?.map(Number) ?? []
      const from: Point = [numbers[0], numbers[1]]
      const to: Point = [numbers[numbers.length - 2], numbers[numbers.length - 1]]
      const tips = [...(path.parentElement?.children ?? [])]
        .filter((sibling) => sibling.matches(".connection[transform]"))
        .map(translationOf)
        .filter((tip) => tip !== null)
      return {
        path,
        from,
        to,
        arrowAtStart: path.hasAttribute("marker-start") || tips.some((tip) => near(tip, from)),
        arrowAtEnd: path.hasAttribute("marker-end") || tips.some((tip) => near(tip, to)),
      }
    })
}

function translationOf(element: Element): Point | null {
  const match = TRANSLATE.exec(element.getAttribute("transform") ?? "")
  return match === null ? null : [Number(match[1]), Number(match[2])]
}

function near([x1, y1]: Point, [x2, y2]: Point): boolean {
  return Math.hypot(x1 - x2, y1 - y2) <= TIP_REACH
}

function paint(element: Element, colour: string): void {
  for (const name of element.classList) {
    const property = THEME_COLOUR.exec(name)?.[1]
    if (property === undefined) continue
    const style = element.getAttribute("style")
    element.setAttribute(
      "style",
      style ? `${style.replace(/;\s*$/, "")};${property}:${colour}` : `${property}:${colour}`,
    )
  }
}

// Markers are shared by every connection that uses them, so each connection gets its own copy.
function recolourMarker(doc: Document, path: Element, end: string, suffix: string, colour: string): void {
  const id = MARKER_URL.exec(path.getAttribute(end) ?? "")?.[1]
  const marker = id === undefined ? null : doc.querySelector(`marker[id="${id}"]`)
  if (marker === null) return
  const copy = marker.cloneNode(true) as Element
  copy.setAttribute("id", `${id}-${suffix}`)
  for (const part of copy.querySelectorAll(".connection")) paint(part, colour)
  marker.after(copy)
  path.setAttribute(end, `url(#${id}-${suffix})`)
}

function dot(doc: Document, [cx, cy]: Point, colour: string): Element {
  const circle = doc.createElementNS(SVG_NS, "circle")
  circle.setAttribute("cx", `${cx}`)
  circle.setAttribute("cy", `${cy}`)
  circle.setAttribute("r", "4")
  circle.setAttribute("style", `fill:${colour}`)
  return circle
}
