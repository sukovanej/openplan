import type { Flow, FlowNode } from "@open-planner/api-client"

export const NODE_WIDTH = 244
export const NODE_HEIGHT = 60
export const COLUMN_STRIDE = NODE_WIDTH + 76
export const WAVE_HEADER_HEIGHT = 20

const ROW_GAP = 14
const BOX_PAD = 12
export const BOX_HEADER = 30
// A wave is a column and the endpoint gives no wave to an unresolved node, so it takes a gutter of
// its own left of the first wave rather than a number it does not have.
const GUTTER = -1

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
}

export interface WaveColumn {
  readonly key: string
  readonly wave: number | undefined
  readonly x: number
}

export interface FlowLayout {
  readonly nodes: ReadonlyArray<PlacedNode>
  readonly edges: ReadonlyArray<PlacedEdge>
  readonly columns: ReadonlyArray<WaveColumn>
}

// A key is unique inside one project only, so a node is named by the pair. A task id holds no
// space, which keeps the join one-to-one.
export function flowNodeKey(project: string, id: string): string {
  return `${project} ${id}`
}

export function columnX(wave: number): number {
  return wave * COLUMN_STRIDE
}

interface Entry {
  readonly key: string
  readonly node: FlowNode
  readonly children: Array<Entry>
  wStart: number
  wEnd: number
  height: number
  y: number
}

interface Rect {
  readonly left: number
  readonly top: number
  readonly right: number
  readonly bottom: number
}

interface Draft {
  readonly entry: Entry
  readonly parent: string | undefined
  readonly depth: number
  rect: Rect
}

const ORIGIN: Rect = { left: 0, top: 0, right: 0, bottom: 0 }

function parentKeyOf(node: FlowNode): string | undefined {
  return node.kind === "unresolved" || node.parent === undefined ? undefined : flowNodeKey(node.project, node.parent)
}

function family(flow: Flow): ReadonlyArray<Entry> {
  const all = new Map<string, Entry>()
  for (const node of flow.nodes) {
    const key = flowNodeKey(node.project, node.id)
    all.set(key, { key, node, children: [], wStart: GUTTER, wEnd: GUTTER, height: NODE_HEIGHT, y: 0 })
  }
  const roots: Array<Entry> = []
  // The endpoint sends the nodes in reading order and puts a box before the leaves it holds, so
  // insertion order carries the order of the waves and of the positions inside them.
  for (const entry of all.values()) {
    const parent = parentKeyOf(entry.node)
    const above = parent === undefined ? undefined : all.get(parent)
    if (above === undefined) roots.push(entry)
    else above.children.push(entry)
  }
  return roots
}

function overlap(a: Entry, b: Entry): boolean {
  return a.wEnd >= b.wStart && b.wEnd >= a.wStart
}

// The first row that leaves this entry clear of every entry it shares a column with. Two entries
// that share no column share a row, which keeps a chain of dependencies on one line.
function firstFit(entry: Entry, placed: ReadonlyArray<Entry>): number {
  let y = 0
  for (const other of [...placed].sort((a, b) => a.y - b.y)) {
    const clash = overlap(entry, other) && y < other.y + other.height + ROW_GAP && other.y < y + entry.height + ROW_GAP
    if (clash) y = other.y + other.height + ROW_GAP
  }
  return y
}

function pack(entries: ReadonlyArray<Entry>): number {
  const placed: Array<Entry> = []
  let height = 0
  for (const entry of entries) {
    entry.y = firstFit(entry, placed)
    placed.push(entry)
    height = Math.max(height, entry.y + entry.height)
  }
  return height
}

function measure(entry: Entry): void {
  if (entry.children.length === 0) {
    const wave = entry.node.kind === "leaf" ? entry.node.wave : GUTTER
    entry.wStart = wave
    entry.wEnd = wave
    entry.height = NODE_HEIGHT
    return
  }
  for (const child of entry.children) measure(child)
  entry.wStart = Math.min(...entry.children.map((child) => child.wStart))
  entry.wEnd = Math.max(...entry.children.map((child) => child.wEnd))
  entry.height = BOX_HEADER + pack(entry.children) + BOX_PAD
}

function union(a: Rect, b: Rect): Rect {
  return {
    left: Math.min(a.left, b.left),
    top: Math.min(a.top, b.top),
    right: Math.max(a.right, b.right),
    bottom: Math.max(a.bottom, b.bottom),
  }
}

// Every node sits in the column of its own wave, and a box grows outwards from the children it
// holds. A box therefore never pushes a child off the grid, however deep the boxes nest.
function draw(entry: Entry, top: number, depth: number, parent: string | undefined, out: Array<Draft>): Rect {
  if (entry.children.length === 0) {
    const left = columnX(entry.wStart)
    const rect = { left, top, right: left + NODE_WIDTH, bottom: top + NODE_HEIGHT }
    out.push({ entry, parent, depth, rect })
    return rect
  }
  // A box goes into the list before the children it holds, which is the order React Flow reads a
  // parent in.
  const draft: Draft = { entry, parent, depth, rect: ORIGIN }
  out.push(draft)
  const content = top + BOX_HEADER
  const held = entry.children.map((child) => draw(child, content + child.y, depth + 1, entry.key, out))
  const bounds = held.reduce(union)
  draft.rect = {
    left: bounds.left - BOX_PAD,
    top,
    right: bounds.right + BOX_PAD,
    bottom: bounds.bottom + BOX_PAD,
  }
  return draft.rect
}

function columns(flow: Flow): ReadonlyArray<WaveColumn> {
  const waves = new Set<number>()
  let gutter = false
  for (const node of flow.nodes) {
    if (node.kind === "leaf") waves.add(node.wave)
    if (node.kind === "unresolved") gutter = true
  }
  const numbered = [...waves].sort((a, b) => a - b).map((wave) => ({ key: String(wave), wave, x: columnX(wave) }))
  return gutter ? [{ key: "unresolved", wave: undefined, x: columnX(GUTTER) }, ...numbered] : numbered
}

export function layoutFlow(flow: Flow): FlowLayout {
  const roots = family(flow)
  for (const root of roots) measure(root)
  pack(roots)

  const drafts: Array<Draft> = []
  for (const root of roots) draw(root, root.y, 0, undefined, drafts)
  const byKey = new Map(drafts.map((draft) => [draft.entry.key, draft]))

  return {
    nodes: drafts.map((draft) => {
      const frame = draft.parent === undefined ? ORIGIN : byKey.get(draft.parent)!.rect
      return {
        key: draft.entry.key,
        node: draft.entry.node,
        parent: draft.parent,
        depth: draft.depth,
        x: draft.rect.left - frame.left,
        y: draft.rect.top - frame.top,
        width: draft.rect.right - draft.rect.left,
        height: draft.rect.bottom - draft.rect.top,
      }
    }),
    edges: flow.edges.map((edge) => ({
      key: `${edge.project} ${edge.from} ${edge.to}`,
      source: flowNodeKey(edge.project, edge.from),
      target: flowNodeKey(edge.project, edge.to),
    })),
    columns: columns(flow),
  }
}
