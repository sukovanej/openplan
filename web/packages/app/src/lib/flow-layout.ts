import type { ElkExtendedEdge, ElkNode } from "elkjs/lib/elk-api"

import type { Flow, FlowEdge, FlowNode } from "@open-planner/api-client"

export const NODE_WIDTH = 244
export const BOX_HEADER = 30
// The title runs to as many lines as it needs, so a card is as tall as its own words. These are the
// numbers the card is drawn with as well as measured by, so the two cannot drift apart.
export const TITLE_SIZE = 14
export const TITLE_LINE = 19
export const TITLE_INSET = 50
export const CARD_PAD = 8
export const KEY_LINE = 16
const MIN_CARD_HEIGHT = 60
const TITLE_FONT = `${TITLE_SIZE}px "Geist Variable", ui-sans-serif, system-ui, sans-serif`

const BOX_PAD = 12
const ISLAND_GAP = 48
// ELK reads the layer of a node from the x it is given, so the stride only has to keep two waves
// apart. The width it lays out is its own.
const LAYER_STRIDE = NODE_WIDTH + 1
// An unresolved node belongs to no wave. It takes the layer left of the first, which no task holds.
const GUTTER = -1

const LAYOUT_OPTIONS: Record<string, string> = {
  "elk.algorithm": "layered",
  "elk.direction": "RIGHT",
  "elk.edgeRouting": "ORTHOGONAL",
  "elk.hierarchyHandling": "INCLUDE_CHILDREN",
  // The endpoint owns the order, so ELK reads each layer from the x of its node rather than
  // computing a layering of its own. The picture and the `wave` field then cannot disagree.
  "elk.layered.layering.strategy": "INTERACTIVE",
  "elk.spacing.nodeNode": "24",
  "elk.spacing.edgeNode": "20",
  "elk.layered.spacing.nodeNodeBetweenLayers": "104",
  "elk.layered.spacing.edgeNodeBetweenLayers": "24",
  "elk.padding": "[top=4,left=4,bottom=4,right=4]",
}

export interface Point {
  readonly x: number
  readonly y: number
}

export interface PlacedNode {
  readonly key: string
  readonly node: FlowNode
  readonly parent: string | undefined
  readonly depth: number
  readonly x: number
  readonly y: number
  readonly width: number
  readonly height: number
}

export interface PlacedEdge {
  readonly key: string
  readonly source: string
  readonly target: string
  readonly points: ReadonlyArray<Point>
}

export interface FlowLayout {
  readonly nodes: ReadonlyArray<PlacedNode>
  readonly edges: ReadonlyArray<PlacedEdge>
}

// A key is unique inside one project only, so a node is named by the pair. A task id holds no
// space, which keeps the join one-to-one.
export function flowNodeKey(project: string, id: string): string {
  return `${project} ${id}`
}

const edgeKey = (edge: FlowEdge): string => `${edge.project} ${edge.from} ${edge.to}`

let ruler: CanvasRenderingContext2D | null | undefined

function measurer(): CanvasRenderingContext2D | null {
  if (ruler === undefined) {
    ruler = (typeof document === "undefined" ? null : document.createElement("canvas").getContext("2d")) ?? null
    if (ruler !== null) ruler.font = TITLE_FONT
  }
  return ruler
}

// Off a canvas — a test, a server — half the type size is close enough to keep the geometry sane.
const widthOf = (text: string): number => measurer()?.measureText(text).width ?? text.length * TITLE_SIZE * 0.5

function titleLines(title: string, width: number): number {
  let lines = 1
  let line = ""
  for (const word of title.split(/\s+/).filter((word) => word !== "")) {
    const next = line === "" ? word : `${line} ${word}`
    if (widthOf(next) <= width || line === "") {
      line = next
      continue
    }
    lines += 1
    line = word
  }
  // A word wider than the card breaks inside itself rather than running out of the frame.
  const longest = Math.max(...title.split(/\s+/).map((word) => Math.ceil(widthOf(word) / width)), 1)
  return lines + longest - 1
}

export function cardHeight(title: string): number {
  const lines = titleLines(title, NODE_WIDTH - TITLE_INSET)
  return Math.max(MIN_CARD_HEIGHT, CARD_PAD * 2 + KEY_LINE + lines * TITLE_LINE)
}

interface Entry {
  readonly key: string
  readonly node: FlowNode
  readonly children: Array<Entry>
}

interface Family {
  readonly roots: ReadonlyArray<Entry>
  readonly parentOf: ReadonlyMap<string, string>
}

function parentKeyOf(node: FlowNode): string | undefined {
  return node.kind === "unresolved" || node.parent === undefined ? undefined : flowNodeKey(node.project, node.parent)
}

function family(flow: Flow): Family {
  const all = new Map<string, Entry>()
  for (const node of flow.nodes) {
    const key = flowNodeKey(node.project, node.id)
    all.set(key, { key, node, children: [] })
  }
  const roots: Array<Entry> = []
  const parentOf = new Map<string, string>()
  // The endpoint sends the nodes in reading order and puts a box before the leaves it holds, so
  // insertion order carries the order of the waves and of the positions inside them.
  for (const entry of all.values()) {
    const parent = parentKeyOf(entry.node)
    const above = parent === undefined ? undefined : all.get(parent)
    if (above === undefined) {
      roots.push(entry)
    } else {
      above.children.push(entry)
      parentOf.set(entry.key, above.key)
    }
  }
  return { roots, parentOf }
}

function ancestorOf(key: string, parentOf: ReadonlyMap<string, string>): string {
  let at = key
  for (let up = parentOf.get(at); up !== undefined; up = parentOf.get(at)) at = up
  return at
}

// The deepest box that holds both ends of an edge. ELK reads the coordinates of an edge against the
// node it is declared in, and an edge declared above the box that holds its own ends is routed
// around that box.
function containerOf(edge: FlowEdge, parentOf: ReadonlyMap<string, string>): string | undefined {
  const above = new Set<string>()
  for (let at = parentOf.get(flowNodeKey(edge.project, edge.from)); at !== undefined; at = parentOf.get(at)) {
    above.add(at)
  }
  for (let at = parentOf.get(flowNodeKey(edge.project, edge.to)); at !== undefined; at = parentOf.get(at)) {
    if (above.has(at)) return at
  }
  return undefined
}

function waveOf(entry: Entry): number {
  if (entry.children.length === 0) return entry.node.kind === "leaf" ? entry.node.wave : GUTTER
  return Math.min(...entry.children.map(waveOf))
}

// Two tasks with no path between them say nothing about each other, so each island is laid out on
// its own and the islands are packed into the shape of the page.
function islands(tree: Family, edges: ReadonlyArray<FlowEdge>): ReadonlyArray<ReadonlyArray<Entry>> {
  const owner = new Map<string, string>(tree.roots.map((entry) => [entry.key, entry.key]))
  const find = (key: string): string => {
    let at = key
    while (owner.get(at) !== at) at = owner.get(at)!
    return at
  }
  for (const edge of edges) {
    const from = ancestorOf(flowNodeKey(edge.project, edge.from), tree.parentOf)
    const to = ancestorOf(flowNodeKey(edge.project, edge.to), tree.parentOf)
    if (owner.has(from) && owner.has(to)) owner.set(find(from), find(to))
  }
  const grouped = new Map<string, Array<Entry>>()
  for (const entry of tree.roots) {
    const key = find(entry.key)
    const held = grouped.get(key)
    if (held === undefined) grouped.set(key, [entry])
    else held.push(entry)
  }
  return [...grouped.values()]
}

function elkNode(entry: Entry, into: Map<string, ElkNode>): ElkNode {
  const x = waveOf(entry) * LAYER_STRIDE
  const node: ElkNode =
    entry.children.length === 0
      ? {
          id: entry.key,
          x,
          y: 0,
          width: NODE_WIDTH,
          height: entry.node.kind === "leaf" ? cardHeight(entry.node.title) : MIN_CARD_HEIGHT,
        }
      : {
          id: entry.key,
          x,
          y: 0,
          layoutOptions: {
            "elk.padding": `[top=${BOX_HEADER},left=${BOX_PAD},bottom=${BOX_PAD},right=${BOX_PAD}]`,
          },
          children: entry.children.map((child) => elkNode(child, into)),
        }
  into.set(entry.key, node)
  return node
}

const ROOT = "root"

function graphOf(island: ReadonlyArray<Entry>, edges: ReadonlyArray<FlowEdge>, tree: Family): ElkNode {
  const byKey = new Map<string, ElkNode>()
  const graph: ElkNode = {
    id: ROOT,
    layoutOptions: LAYOUT_OPTIONS,
    children: island.map((entry) => elkNode(entry, byKey)),
    edges: [],
  }
  for (const edge of edges) {
    const source = flowNodeKey(edge.project, edge.from)
    const target = flowNodeKey(edge.project, edge.to)
    if (!byKey.has(source) || !byKey.has(target)) continue
    const container = containerOf(edge, tree.parentOf)
    const holder = container === undefined ? graph : byKey.get(container)!
    holder.edges ??= []
    holder.edges.push({ id: edgeKey(edge), sources: [source], targets: [target] })
  }
  return graph
}

interface Size {
  readonly width: number
  readonly height: number
}

interface Shelved {
  readonly places: ReadonlyArray<Point>
  readonly width: number
  readonly height: number
}

function shelve(sizes: ReadonlyArray<Size>, limit: number): Shelved {
  const places: Array<Point> = []
  let x = 0
  let y = 0
  let shelf = 0
  let width = 0
  for (const size of sizes) {
    if (x > 0 && x + size.width > limit) {
      y += shelf + ISLAND_GAP
      x = 0
      shelf = 0
    }
    places.push({ x, y })
    x += size.width + ISLAND_GAP
    shelf = Math.max(shelf, size.height)
    width = Math.max(width, x - ISLAND_GAP)
  }
  return { places, width, height: y + shelf }
}

// The islands go into rows as wide as the page is shaped, so the whole graph reads at one zoom
// instead of running off the bottom in a single column.
export function pack(sizes: ReadonlyArray<Size>, aspectRatio: number): ReadonlyArray<Point> {
  if (sizes.length === 0) return []
  const widest = Math.max(...sizes.map((size) => size.width))
  const spread = sizes.reduce((sum, size) => sum + size.width + ISLAND_GAP, 0)
  const tries = 32
  let best = shelve(sizes, widest)
  let missed = Infinity
  for (let step = 0; step <= tries; step++) {
    const shelved = shelve(sizes, widest + ((spread - widest) * step) / tries)
    const off = Math.abs(Math.log(shelved.width / Math.max(shelved.height, 1) / aspectRatio))
    if (off < missed) {
      missed = off
      best = shelved
    }
  }
  return best.places
}

interface Collected {
  readonly nodes: Array<PlacedNode>
  readonly edges: Array<PlacedEdge>
  readonly origins: Map<string, Point>
}

function collect(
  node: ElkNode,
  entries: ReadonlyMap<string, Entry>,
  parent: string | undefined,
  depth: number,
  origin: Point,
  out: Collected,
): void {
  const at = { x: origin.x + (node.x ?? 0), y: origin.y + (node.y ?? 0) }
  out.origins.set(node.id, at)
  out.nodes.push({
    key: node.id,
    node: entries.get(node.id)!.node,
    parent,
    depth,
    // A root carries the offset its island was packed to. A child stays where its own box holds it,
    // which is the position React Flow reads.
    x: parent === undefined ? at.x : (node.x ?? 0),
    y: parent === undefined ? at.y : (node.y ?? 0),
    width: node.width ?? NODE_WIDTH,
    height: node.height ?? MIN_CARD_HEIGHT,
  })
  for (const child of node.children ?? []) collect(child, entries, node.id, depth + 1, at, out)
}

function wires(node: ElkNode, out: Collected): void {
  for (const edge of (node.edges ?? []) as Array<ElkExtendedEdge>) {
    const origin = out.origins.get(edge.container ?? node.id) ?? { x: 0, y: 0 }
    const section = edge.sections?.[0]
    if (section === undefined) continue
    const points = [section.startPoint, ...(section.bendPoints ?? []), section.endPoint]
    out.edges.push({
      key: edge.id,
      source: edge.sources[0],
      target: edge.targets[0],
      points: points.map((point) => ({ x: origin.x + point.x, y: origin.y + point.y })),
    })
  }
  for (const child of node.children ?? []) wires(child, out)
}

type Engine = { layout: (graph: ElkNode) => Promise<ElkNode> }

let started: Promise<Engine> | undefined

// About 1.5 MB of layout engine, fetched when a reader opens the flow rather than with the app.
function engine(): Promise<Engine> {
  started ??= import("elkjs/lib/elk.bundled.js").then((module) => new module.default())
  return started
}

function indexEntries(entry: Entry, into: Map<string, Entry>): void {
  into.set(entry.key, entry)
  for (const child of entry.children) indexEntries(child, into)
}

export async function layoutFlow(flow: Flow, aspectRatio: number): Promise<FlowLayout> {
  // The card is measured in the font it is drawn in, which the browser may still be fetching.
  if (typeof document !== "undefined") await document.fonts?.ready
  const tree = family(flow)
  const entries = new Map<string, Entry>()
  for (const root of tree.roots) indexEntries(root, entries)

  const elk = await engine()
  const laid = await Promise.all(
    islands(tree, flow.edges).map((island) => elk.layout(graphOf(island, flow.edges, tree))),
  )
  const places = pack(
    laid.map((graph) => ({ width: graph.width ?? 0, height: graph.height ?? 0 })),
    aspectRatio,
  )

  const out: Collected = { nodes: [], edges: [], origins: new Map() }
  laid.forEach((graph, at) => {
    out.origins.set(ROOT, places[at])
    for (const child of graph.children ?? []) collect(child, entries, undefined, 0, places[at], out)
    wires(graph, out)
  })
  return { nodes: out.nodes, edges: out.edges }
}
