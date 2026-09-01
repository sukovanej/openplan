import { describe, expect, it } from "vitest"

import type { Flow, FlowNode } from "@open-planner/api-client"

import {
  cardHeight,
  flowNodeKey,
  type FlowLayout,
  layoutFlow,
  pack,
  type PlacedNode,
  TITLE_LINE,
} from "../src/lib/flow-layout"

const PROJECT = "open-plan"
const PAGE = 1.8

function leaf(id: string, wave: number, position: number, parent?: string, title = id): FlowNode {
  return { kind: "leaf", project: PROJECT, id, title, status: "todo", parent, wave, position, blocks_count: 0 }
}

function box(id: string, parent?: string): FlowNode {
  return { kind: "box", project: PROJECT, id, title: id, status: "todo", parent }
}

function unresolved(id: string): FlowNode {
  return { kind: "unresolved", project: PROJECT, id }
}

function edge(from: string, to: string) {
  return { project: PROJECT, from, to }
}

interface Frame {
  readonly left: number
  readonly top: number
  readonly right: number
  readonly bottom: number
}

function frames(layout: FlowLayout): Map<string, Frame> {
  const byKey = new Map(layout.nodes.map((node) => [node.key, node]))
  const origin = (node: PlacedNode): { x: number; y: number } => {
    if (node.parent === undefined) return { x: node.x, y: node.y }
    const above = origin(byKey.get(node.parent)!)
    return { x: above.x + node.x, y: above.y + node.y }
  }
  return new Map(
    layout.nodes.map((node) => {
      const { x, y } = origin(node)
      return [node.key, { left: x, top: y, right: x + node.width, bottom: y + node.height }]
    }),
  )
}

const at = (placed: Map<string, Frame>, id: string): Frame => placed.get(flowNodeKey(PROJECT, id))!

function holds(outer: Frame, inner: Frame): boolean {
  return outer.left <= inner.left && outer.top < inner.top && outer.right >= inner.right && outer.bottom >= inner.bottom
}

function overlaps(a: Frame, b: Frame): boolean {
  return a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom
}

// Every node a line crosses on its way, which is the thing a routed edge must never do.
function crossings(layout: FlowLayout): Array<string> {
  const placed = frames(layout)
  const cards = layout.nodes.filter((node) => node.node.kind !== "box")
  const hits: Array<string> = []
  for (const wire of layout.edges) {
    for (let step = 0; step < wire.points.length - 1; step++) {
      const from = wire.points[step]
      const to = wire.points[step + 1]
      for (let cut = 0; cut <= 40; cut++) {
        const x = from.x + ((to.x - from.x) * cut) / 40
        const y = from.y + ((to.y - from.y) * cut) / 40
        for (const card of cards) {
          if (card.key === wire.source || card.key === wire.target) continue
          const frame = placed.get(card.key)!
          if (x > frame.left + 1 && x < frame.right - 1 && y > frame.top + 1 && y < frame.bottom - 1) {
            hits.push(`${wire.key} over ${card.key}`)
          }
        }
      }
    }
  }
  return [...new Set(hits)]
}

function collisions(layout: FlowLayout): Array<string> {
  const placed = frames(layout)
  const byKey = new Map(layout.nodes.map((node) => [node.key, node]))
  const encloses = (outer: string, inner: string): boolean => {
    for (let up = byKey.get(inner)?.parent; up !== undefined; up = byKey.get(up)?.parent) {
      if (up === outer) return true
    }
    return false
  }
  const hits: Array<string> = []
  for (const a of layout.nodes) {
    for (const b of layout.nodes) {
      if (a.key >= b.key) continue
      if (encloses(a.key, b.key) || encloses(b.key, a.key)) continue
      if (overlaps(placed.get(a.key)!, placed.get(b.key)!)) hits.push(`${a.key} over ${b.key}`)
    }
  }
  return hits
}

describe("the drawing", () => {
  it("puts a task to the right of everything it waits for", async () => {
    const flow: Flow = {
      nodes: [leaf("OPP-1", 0, 0), leaf("OPP-2", 1, 0), leaf("OPP-3", 2, 0)],
      edges: [edge("OPP-1", "OPP-2"), edge("OPP-2", "OPP-3")],
    }
    const placed = frames(await layoutFlow(flow, PAGE))
    expect(at(placed, "OPP-1").right).toBeLessThan(at(placed, "OPP-2").left)
    expect(at(placed, "OPP-2").right).toBeLessThan(at(placed, "OPP-3").left)
  })

  it("keeps two tasks of one wave in the same column", async () => {
    const flow: Flow = {
      nodes: [leaf("OPP-1", 0, 0), leaf("OPP-2", 0, 1), leaf("OPP-3", 1, 0)],
      edges: [edge("OPP-1", "OPP-3"), edge("OPP-2", "OPP-3")],
    }
    const placed = frames(await layoutFlow(flow, PAGE))
    expect(at(placed, "OPP-1").left).toBe(at(placed, "OPP-2").left)
    expect(at(placed, "OPP-1").left).toBeLessThan(at(placed, "OPP-3").left)
  })

  it("leaves a node with no wave to the left of the task that names it", async () => {
    const flow: Flow = {
      nodes: [leaf("OPP-1", 0, 0), unresolved("OPP-99")],
      edges: [edge("OPP-99", "OPP-1")],
    }
    const placed = frames(await layoutFlow(flow, PAGE))
    expect(at(placed, "OPP-99").right).toBeLessThan(at(placed, "OPP-1").left)
  })

  it("holds each child inside its box", async () => {
    const flow: Flow = {
      nodes: [box("OPP-40"), leaf("OPP-41", 0, 0, "OPP-40"), leaf("OPP-42", 1, 0, "OPP-40"), leaf("OPP-50", 2, 0)],
      edges: [edge("OPP-41", "OPP-42"), edge("OPP-40", "OPP-50")],
    }
    const placed = frames(await layoutFlow(flow, PAGE))
    expect(holds(at(placed, "OPP-40"), at(placed, "OPP-41"))).toBe(true)
    expect(holds(at(placed, "OPP-40"), at(placed, "OPP-42"))).toBe(true)
    expect(overlaps(at(placed, "OPP-40"), at(placed, "OPP-50"))).toBe(false)
  })

  it("nests a box inside a box", async () => {
    const flow: Flow = {
      nodes: [box("OPP-10"), box("OPP-20", "OPP-10"), leaf("OPP-21", 0, 0, "OPP-20"), leaf("OPP-11", 1, 0, "OPP-10")],
      edges: [edge("OPP-21", "OPP-11")],
    }
    const placed = frames(await layoutFlow(flow, PAGE))
    expect(holds(at(placed, "OPP-10"), at(placed, "OPP-20"))).toBe(true)
    expect(holds(at(placed, "OPP-20"), at(placed, "OPP-21"))).toBe(true)
    expect(holds(at(placed, "OPP-10"), at(placed, "OPP-11"))).toBe(true)
  })

  it("puts a box before the children it holds, which is the order React Flow reads", async () => {
    const flow: Flow = {
      nodes: [box("OPP-40"), leaf("OPP-41", 0, 0, "OPP-40"), leaf("OPP-50", 0, 1)],
      edges: [],
    }
    const keys = (await layoutFlow(flow, PAGE)).nodes.map((node) => node.node.id)
    expect(keys.indexOf("OPP-40")).toBeLessThan(keys.indexOf("OPP-41"))
  })

  it("leaves a task whose parent the flow does not hold at the top level", async () => {
    const flow: Flow = { nodes: [leaf("OPP-41", 0, 0, "OPP-40")], edges: [] }
    const [node] = (await layoutFlow(flow, PAGE)).nodes
    expect(node.parent).toBeUndefined()
    expect(node.depth).toBe(0)
  })
})

describe("nothing overlaps", () => {
  // A long dependency skips a wave, which is what puts a task under the line if nothing routes it.
  const skipping: Flow = {
    nodes: [
      leaf("OPP-1", 0, 0),
      leaf("OPP-2", 1, 0),
      leaf("OPP-3", 1, 1),
      leaf("OPP-4", 2, 0),
      leaf("OPP-5", 3, 0),
      leaf("OPP-6", 3, 1),
    ],
    edges: [
      edge("OPP-1", "OPP-2"),
      edge("OPP-1", "OPP-3"),
      edge("OPP-2", "OPP-4"),
      edge("OPP-4", "OPP-5"),
      edge("OPP-1", "OPP-6"),
      edge("OPP-3", "OPP-5"),
    ],
  }

  it("draws no line across a task it does not join", async () => {
    expect(crossings(await layoutFlow(skipping, PAGE))).toEqual([])
  })

  it("leaves no two nodes on the same pixels", async () => {
    expect(collisions(await layoutFlow(skipping, PAGE))).toEqual([])
  })

  it("keeps a line clear of the boxes it passes", async () => {
    const flow: Flow = {
      nodes: [
        box("OPP-40"),
        leaf("OPP-41", 0, 0, "OPP-40"),
        leaf("OPP-42", 1, 0, "OPP-40"),
        leaf("OPP-50", 0, 1),
        leaf("OPP-51", 3, 0),
      ],
      edges: [edge("OPP-41", "OPP-42"), edge("OPP-50", "OPP-51"), edge("OPP-40", "OPP-51")],
    }
    const layout = await layoutFlow(flow, PAGE)
    expect(crossings(layout)).toEqual([])
    expect(collisions(layout)).toEqual([])
  })
})

describe("a card grows with its title", () => {
  const short = "Ship it"
  const long =
    "Section-level markdown editing of task bodies, from the engine through the CLI and the API to the web editor"

  it("gives a long title more room than a short one", () => {
    expect(cardHeight(long)).toBeGreaterThan(cardHeight(short))
  })

  it("counts the lines of every word too wide for the card, not just the widest", () => {
    const wide = "x".repeat(60)
    expect(cardHeight(`${wide} ${wide}`)).toBeGreaterThan(cardHeight(wide) + TITLE_LINE)
  })

  it("holds a short title at the smallest card", () => {
    expect(cardHeight(short)).toBe(cardHeight(""))
  })

  it("draws the card as tall as the layout measured it", async () => {
    const flow: Flow = { nodes: [leaf("OPP-1", 0, 0), leaf("OPP-2", 0, 1, undefined, long)], edges: [] }
    const placed = (await layoutFlow(flow, PAGE)).nodes
    expect(placed.find((node) => node.node.id === "OPP-2")!.height).toBe(cardHeight(long))
    expect(placed.find((node) => node.node.id === "OPP-1")!.height).toBe(cardHeight("OPP-1"))
  })
})

describe("islands fill the page", () => {
  const island = (n: number) => Array.from({ length: n }, () => ({ width: 244, height: 60 }))

  it("lays a pile of lone tasks out wide when the page is wide", () => {
    const places = pack(island(15), 4)
    const rows = new Set(places.map((place) => place.y))
    const columns = new Set(places.map((place) => place.x))
    expect(columns.size).toBeGreaterThan(1)
    expect(rows.size).toBeLessThan(15)
  })

  it("lays the same pile out tall when the page is tall", () => {
    const wide = pack(island(15), 4)
    const tall = pack(island(15), 0.3)
    expect(new Set(tall.map((place) => place.x)).size).toBeLessThan(new Set(wide.map((place) => place.x)).size)
  })

  it("packs one island at the origin", () => {
    expect(pack(island(1), 1.8)).toEqual([{ x: 0, y: 0 }])
    expect(pack([], 1.8)).toEqual([])
  })
})
