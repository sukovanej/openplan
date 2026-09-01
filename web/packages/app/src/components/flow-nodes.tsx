import { Handle, type Node, type NodeProps, Position } from "@xyflow/react"
import { Link } from "react-router-dom"

import type { FlowNode } from "@open-planner/api-client"
import { fieldValue, statusBorder, StatusMark, taskPath, UnresolvedMark } from "@open-planner/task-ui"
import { cn } from "@open-planner/ui"

import { BOX_HEADER } from "../lib/flow-layout"

type Leaf = Extract<FlowNode, { kind: "leaf" }>
type Box = Extract<FlowNode, { kind: "box" }>
type Unresolved = Extract<FlowNode, { kind: "unresolved" }>

export type LeafFlowNode = Node<{ task: Leaf }, "leaf">
export type BoxFlowNode = Node<{ task: Box }, "box">
export type UnresolvedFlowNode = Node<{ reference: Unresolved }, "unresolved">

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
      <Link
        to={taskPath(task.project, task.id)}
        className={cn(
          "bg-card hover:bg-muted/40 flex h-full w-full items-center gap-2.5 rounded-md border px-3 py-2 transition-colors",
          statusBorder(fieldValue(task.status)),
        )}
      >
        <StatusMark status={task.status} className="size-4 shrink-0" />
        <span className="flex min-w-0 flex-col gap-0.5">
          <span className="text-muted-foreground text-[11px] tabular-nums">{task.id}</span>
          <span className="text-foreground/90 line-clamp-2 text-sm leading-snug">{task.title}</span>
        </span>
      </Link>
    </>
  )
}

export function BoxFlowCard({ data }: NodeProps<BoxFlowNode>) {
  const { task } = data
  return (
    <>
      <Sides />
      <div className={cn("bg-muted/20 h-full w-full rounded-lg border", statusBorder(fieldValue(task.status)))}>
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
