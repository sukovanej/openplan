import { BaseEdge, type Edge, type EdgeProps, Handle, type Node, type NodeProps, Position } from "@xyflow/react"
import { Link } from "react-router-dom"

import type { FlowNode } from "@open-planner/api-client"
import {
  fieldValue,
  statusBorder,
  statusFaint,
  StatusMark,
  statusSurface,
  taskPath,
  UnresolvedMark,
} from "@open-planner/task-ui"
import { cn } from "@open-planner/ui"

import { BOX_HEADER, CARD_PAD, KEY_LINE, type Point, TITLE_LINE, TITLE_SIZE } from "../lib/flow-layout"

type Leaf = Extract<FlowNode, { kind: "leaf" }>
type Box = Extract<FlowNode, { kind: "box" }>
type Unresolved = Extract<FlowNode, { kind: "unresolved" }>

export type LeafFlowNode = Node<{ task: Leaf }, "leaf">
export type BoxFlowNode = Node<{ task: Box }, "box">
export type UnresolvedFlowNode = Node<{ reference: Unresolved }, "unresolved">
export type RoutedFlowEdge = Edge<{ points: ReadonlyArray<Point> }, "routed">

// A handle is where an edge meets a node. React Flow measures it from the DOM, so it stays in the
// layout and only its paint goes.
function Sides() {
  return (
    <>
      <Handle type="target" position={Position.Left} isConnectable={false} className="opacity-0" />
      <Handle type="source" position={Position.Right} isConnectable={false} className="opacity-0" />
    </>
  )
}

export function LeafFlowCard({ data }: NodeProps<LeafFlowNode>) {
  const { task } = data
  return (
    <>
      <Sides />
      {/* The wash is thin enough to read text through, so an opaque card lies under it rather than
          the dotted ground of the page. */}
      <div className="bg-card h-full w-full rounded-md">
        <Link
          to={taskPath(task.project, task.id)}
          style={{ paddingTop: CARD_PAD, paddingBottom: CARD_PAD }}
          className={cn(
            "hover:brightness-110 flex h-full w-full items-center gap-2.5 rounded-md border px-3 transition-[filter]",
            statusBorder(fieldValue(task.status)),
            statusSurface(fieldValue(task.status)),
          )}
        >
          <StatusMark status={task.status} className="size-4 shrink-0" />
          <span className="flex min-w-0 flex-col">
            <span style={{ lineHeight: `${KEY_LINE}px` }} className="text-muted-foreground text-[11px] tabular-nums">
              {task.id}
            </span>
            {/* The layout measured this text to size the card, so it wraps here and never clips. */}
            <span
              style={{ fontSize: TITLE_SIZE, lineHeight: `${TITLE_LINE}px` }}
              className="text-foreground/90 break-words"
            >
              {task.title}
            </span>
          </span>
        </Link>
      </div>
    </>
  )
}

export function BoxFlowCard({ data }: NodeProps<BoxFlowNode>) {
  const { task } = data
  return (
    <>
      <Sides />
      <div
        className={cn(
          "h-full w-full rounded-lg border",
          statusBorder(fieldValue(task.status)),
          statusFaint(fieldValue(task.status)),
        )}
      >
        <Link
          to={taskPath(task.project, task.id)}
          style={{ height: BOX_HEADER }}
          className="hover:text-foreground flex min-w-0 items-center gap-2 px-3"
        >
          <StatusMark status={task.status} className="size-3.5 shrink-0" />
          <span className="text-muted-foreground shrink-0 text-[11px] tabular-nums">{task.id}</span>
          <span className="text-muted-foreground min-w-0 truncate text-[11px]">{task.title}</span>
        </Link>
      </div>
    </>
  )
}

export function UnresolvedFlowCard({ data }: NodeProps<UnresolvedFlowNode>) {
  const { reference } = data
  return (
    <>
      <Sides />
      <div className="border-muted-foreground/40 bg-background flex h-full w-full items-center gap-2 rounded-md border border-dashed px-3 py-2">
        <UnresolvedMark />
        <span className="text-muted-foreground truncate font-mono text-sm">{reference.id}</span>
      </div>
    </>
  )
}

const CORNER = 10

// The turns the layout engine chose, rounded. React Flow would otherwise draw a line of its own
// between the two handles, and that line is the one that runs over the nodes in between.
function corners(points: ReadonlyArray<Point>): string {
  if (points.length === 0) return ""
  const [first, ...rest] = points
  let path = `M ${first.x} ${first.y}`
  for (let at = 1; at < points.length - 1; at++) {
    const from = points[at - 1]
    const turn = points[at]
    const to = points[at + 1]
    const back = Math.min(CORNER, Math.hypot(turn.x - from.x, turn.y - from.y) / 2)
    const on = Math.min(CORNER, Math.hypot(to.x - turn.x, to.y - turn.y) / 2)
    const enter = { x: turn.x + Math.sign(from.x - turn.x) * back, y: turn.y + Math.sign(from.y - turn.y) * back }
    const leave = { x: turn.x + Math.sign(to.x - turn.x) * on, y: turn.y + Math.sign(to.y - turn.y) * on }
    path += ` L ${enter.x} ${enter.y} Q ${turn.x} ${turn.y} ${leave.x} ${leave.y}`
  }
  const last = rest[rest.length - 1] ?? first
  return `${path} L ${last.x} ${last.y}`
}

export function RoutedFlowLine({ data, markerEnd, style }: EdgeProps<RoutedFlowEdge>) {
  return <BaseEdge path={corners(data?.points ?? [])} markerEnd={markerEnd} style={style} />
}
