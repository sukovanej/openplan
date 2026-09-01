import { describe, expect, it } from "vitest"

import type { Flow, FlowNode } from "@open-planner/api-client"

import { columnX, flowNodeKey, layoutFlow, NODE_HEIGHT, NODE_WIDTH, type PlacedNode } from "../src/lib/flow-layout"

const PROJECT = "open-plan"

function leaf(id: string, wave: number, position: number, parent?: string): FlowNode {
  return { kind: "leaf", project: PROJECT, id, title: id, status: "todo", parent, wave, position, blocks_count: 0 }
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

function frames(flow: Flow): Map<string, Frame> {
  const layout = layoutFlow(flow)
  const byKey = new Map(layout.nodes.map((node) => [node.key, node]))
  const absolute = (node: PlacedNode): { x: number; y: number } => {
    if (node.parent === undefined) return { x: node.x, y: node.y }
    const above = absolute(byKey.get(node.parent)!)
    return { x: above.x + node.x, y: above.y + node.y }
  }
  return new Map(
    layout.nodes.map((node) => {
      const { x, y } = absolute(node)
      return [node.key, { left: x, top: y, right: x + node.width, bottom: y + node.height }]
    }),
  )
}

const at = (frames: Map<string, Frame>, id: string): Frame => frames.get(flowNodeKey(PROJECT, id))!

function holds(outer: Frame, inner: Frame): boolean {
  return outer.left < inner.left && outer.top < inner.top && outer.right > inner.right && outer.bottom > inner.bottom
}

function overlaps(a: Frame, b: Frame): boolean {
  return a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom
}

describe("the grid", () => {
  it("puts the wave in the column and keeps a chain on one row", () => {
    const flow: Flow = {
      nodes: [leaf("OPP-1", 0, 0), leaf("OPP-2", 1, 0)],
      edges: [edge("OPP-1", "OPP-2")],
    }
    const placed = frames(flow)
    expect(at(placed, "OPP-1").left).toBe(columnX(0))
    expect(at(placed, "OPP-2").left).toBe(columnX(1))
    expect(at(placed, "OPP-1").top).toBe(at(placed, "OPP-2").top)
    expect(at(placed, "OPP-1").bottom - at(placed, "OPP-1").top).toBe(NODE_HEIGHT)
    expect(at(placed, "OPP-1").right - at(placed, "OPP-1").left).toBe(NODE_WIDTH)
  })

  it("stacks two tasks of one wave in separate rows", () => {
    const flow: Flow = { nodes: [leaf("OPP-1", 0, 0), leaf("OPP-2", 0, 1)], edges: [] }
    const placed = frames(flow)
    expect(at(placed, "OPP-1").left).toBe(at(placed, "OPP-2").left)
    expect(overlaps(at(placed, "OPP-1"), at(placed, "OPP-2"))).toBe(false)
    expect(at(placed, "OPP-1").top).toBeLessThan(at(placed, "OPP-2").top)
  })

  it("puts a node with no wave in a gutter left of the first one", () => {
    const flow: Flow = {
      nodes: [leaf("OPP-1", 0, 0), leaf("OPP-2", 1, 0), unresolved("WEB-7")],
      edges: [edge("WEB-7", "OPP-2")],
    }
    const placed = frames(flow)
    expect(at(placed, "WEB-7").right).toBeLessThan(columnX(0))
  })
})

describe("a parent as a box", () => {
  it("holds each child inside its frame and reads its span from them", () => {
    const flow: Flow = {
      nodes: [box("OPP-40"), leaf("OPP-41", 0, 0, "OPP-40"), leaf("OPP-42", 1, 0, "OPP-40"), leaf("OPP-50", 2, 0)],
      edges: [edge("OPP-41", "OPP-42"), edge("OPP-40", "OPP-50")],
    }
    const placed = frames(flow)
    expect(holds(at(placed, "OPP-40"), at(placed, "OPP-41"))).toBe(true)
    expect(holds(at(placed, "OPP-40"), at(placed, "OPP-42"))).toBe(true)
    expect(overlaps(at(placed, "OPP-40"), at(placed, "OPP-50"))).toBe(false)
    expect(at(placed, "OPP-40").left).toBeLessThan(columnX(0))
    expect(at(placed, "OPP-40").right).toBeGreaterThan(columnX(1) + NODE_WIDTH)
  })

  it("gives a child a position its own box reads as the origin", () => {
    const flow: Flow = {
      nodes: [box("OPP-40"), leaf("OPP-41", 0, 0, "OPP-40")],
      edges: [],
    }
    const child = layoutFlow(flow).nodes.find((node) => node.node.id === "OPP-41")!
    expect(child.parent).toBe(flowNodeKey(PROJECT, "OPP-40"))
    expect(child.x).toBeGreaterThan(0)
    expect(child.y).toBeGreaterThan(0)
  })

  it("nests a box inside a box", () => {
    const flow: Flow = {
      nodes: [box("OPP-10"), box("OPP-20", "OPP-10"), leaf("OPP-21", 0, 0, "OPP-20"), leaf("OPP-11", 1, 0, "OPP-10")],
      edges: [],
    }
    const placed = frames(flow)
    expect(holds(at(placed, "OPP-10"), at(placed, "OPP-20"))).toBe(true)
    expect(holds(at(placed, "OPP-20"), at(placed, "OPP-21"))).toBe(true)
    expect(holds(at(placed, "OPP-10"), at(placed, "OPP-11"))).toBe(true)
    expect(overlaps(at(placed, "OPP-20"), at(placed, "OPP-11"))).toBe(false)
  })

  it("puts a box before the children it holds, which is the order React Flow reads", () => {
    const flow: Flow = {
      nodes: [box("OPP-40"), leaf("OPP-41", 0, 0, "OPP-40"), leaf("OPP-50", 0, 1)],
      edges: [],
    }
    const keys = layoutFlow(flow).nodes.map((node) => node.node.id)
    expect(keys.indexOf("OPP-40")).toBeLessThan(keys.indexOf("OPP-41"))
  })

  it("leaves a box whose parent the flow does not hold at the top level", () => {
    const flow: Flow = { nodes: [leaf("OPP-41", 0, 0, "OPP-40")], edges: [] }
    const [node] = layoutFlow(flow).nodes
    expect(node.parent).toBeUndefined()
    expect(node.depth).toBe(0)
  })
})

describe("the edges", () => {
  it("joins the nodes of one project by their keys", () => {
    const flow: Flow = {
      nodes: [leaf("OPP-1", 0, 0), leaf("OPP-2", 1, 0)],
      edges: [edge("OPP-1", "OPP-2")],
    }
    expect(layoutFlow(flow).edges).toEqual([
      {
        key: `${PROJECT} OPP-1 OPP-2`,
        source: flowNodeKey(PROJECT, "OPP-1"),
        target: flowNodeKey(PROJECT, "OPP-2"),
      },
    ])
  })

  it("keeps the same key of two projects apart", () => {
    const flow: Flow = {
      nodes: [
        leaf("OPP-1", 0, 0),
        {
          kind: "leaf",
          project: "web",
          id: "OPP-1",
          title: "other",
          status: "todo",
          wave: 0,
          position: 1,
          blocks_count: 0,
        },
      ],
      edges: [],
    }
    const keys = layoutFlow(flow).nodes.map((node) => node.key)
    expect(new Set(keys).size).toBe(2)
  })
})
