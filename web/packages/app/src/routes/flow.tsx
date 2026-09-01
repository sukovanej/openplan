import { useQuery, type UseQueryResult } from "@tanstack/react-query"
import {
  Background,
  BackgroundVariant,
  Controls,
  type Edge,
  MarkerType,
  type Node,
  type NodeTypes,
  ReactFlow,
} from "@xyflow/react"
import { useMemo } from "react"
import { Link, useSearchParams } from "react-router-dom"

import type { Flow } from "@open-planner/api-client"
import { FLOW_ROUTE } from "@open-planner/task-ui"
import { EmptyState, Panel, PanelBody, PanelHeader, PanelTitle, Skeleton } from "@open-planner/ui"

import { BoxFlowCard, LeafFlowCard, UnresolvedFlowCard, WaveFlowLabel } from "../components/flow-nodes"
import { FlowCycles, getFlow } from "../lib/api"
import { type FlowLayout, layoutFlow, NODE_WIDTH, WAVE_HEADER_HEIGHT } from "../lib/flow-layout"
import { describeSelection, readSelection, selectionParams, selectsEveryTask } from "../lib/flow-selection"
import { errorText } from "../lib/format"
import { flowKey } from "../lib/query-client"
import { useRowCursor } from "../lib/row-cursor"
import { runtime } from "../lib/runtime"
import { useTheme } from "../lib/theme"

import "@xyflow/react/dist/style.css"

// This page holds no task rows, and the cursor is the board's — left as it was, `j` then Enter here
// would open a task the reader can no longer see.
const NO_ROWS: ReadonlyArray<string> = []

const NODE_TYPES: NodeTypes = {
  leaf: LeafFlowCard,
  box: BoxFlowCard,
  unresolved: UnresolvedFlowCard,
  wave: WaveFlowLabel,
}

const WAVE_LABEL_GAP = 14

export function FlowRoute() {
  const [params] = useSearchParams()
  const query = params.toString()
  const selection = useMemo(() => readSelection(new URLSearchParams(query)), [query])
  const flow = useQuery({
    queryKey: flowKey(selectionParams(selection).toString()),
    queryFn: () => runtime.runPromise(getFlow(selection)),
  })
  useRowCursor(NO_ROWS)

  return (
    <Panel>
      <PanelHeader className="gap-3">
        <PanelTitle>Flow</PanelTitle>
        <span className="text-muted-foreground min-w-0 truncate text-xs normal-case">
          {describeSelection(selection)}
        </span>
        {!selectsEveryTask(selection) && (
          <Link to={FLOW_ROUTE} className="text-muted-foreground hover:text-foreground ml-auto shrink-0 text-xs">
            Show every task
          </Link>
        )}
      </PanelHeader>
      <PanelBody className="overflow-hidden">
        <FlowState flow={flow} />
      </PanelBody>
    </Panel>
  )
}

function FlowState({ flow }: { flow: UseQueryResult<Flow> }) {
  if (flow.isPending) return <Skeleton className="m-6 h-[calc(100%-3rem)]" />
  if (flow.isError) {
    return (
      <div className="p-6">
        {flow.error instanceof FlowCycles ? (
          <EmptyState title="The dependencies form a cycle" detail={flow.error.cycles.map(round).join("; ")} />
        ) : (
          <EmptyState title="Could not load the flow" detail={errorText(flow.error)} />
        )}
      </div>
    )
  }
  if (flow.data.nodes.length === 0) {
    return (
      <div className="p-6">
        <EmptyState title="Nothing to order" detail="No task matches this flow." />
      </div>
    )
  }
  return <Diagram flow={flow.data} />
}

function round(cycle: ReadonlyArray<string>): string {
  return [...cycle, cycle[0]].join(" → ")
}

function Diagram({ flow }: { flow: Flow }) {
  const { resolved } = useTheme()
  const layout = useMemo(() => layoutFlow(flow), [flow])
  const nodes = useMemo(() => reactFlowNodes(layout), [layout])
  const edges = useMemo(() => reactFlowEdges(layout), [layout])
  return (
    <ReactFlow
      nodes={nodes}
      edges={edges}
      nodeTypes={NODE_TYPES}
      colorMode={resolved}
      fitView
      // A flow of two nodes would otherwise fill the box at the largest zoom the reader can reach.
      fitViewOptions={{ maxZoom: 1, padding: 0.15 }}
      minZoom={0.08}
      maxZoom={1.6}
      // The layout comes from the endpoint, so a node holds still and answers the click that opens
      // its task. React Flow gives a node pointer events only while it is selectable.
      nodesDraggable={false}
      nodesConnectable={false}
      deleteKeyCode={null}
      className="h-full w-full"
    >
      <Background variant={BackgroundVariant.Dots} gap={24} size={1} />
      <Controls showInteractive={false} />
    </ReactFlow>
  )
}

function reactFlowNodes(layout: FlowLayout): Array<Node> {
  const waves: Array<Node> = layout.columns.map((column) => ({
    id: `wave ${column.key}`,
    type: "wave",
    data: { wave: column.wave },
    position: { x: column.x, y: -(WAVE_HEADER_HEIGHT + WAVE_LABEL_GAP) },
    width: NODE_WIDTH,
    height: WAVE_HEADER_HEIGHT,
    selectable: false,
    zIndex: 0,
  }))
  const placed: Array<Node> = layout.nodes.map((node) => ({
    id: node.key,
    type: node.node.kind,
    data: node.node.kind === "unresolved" ? { reference: node.node } : { task: node.node },
    position: { x: node.x, y: node.y },
    width: node.width,
    height: node.height,
    parentId: node.parent,
    extent: node.parent === undefined ? undefined : "parent",
    // A box holds its children, so it has to paint under them.
    zIndex: node.depth + 1,
  }))
  return [...waves, ...placed]
}

function reactFlowEdges(layout: FlowLayout): Array<Edge> {
  return layout.edges.map((edge) => ({
    id: edge.key,
    source: edge.source,
    target: edge.target,
    type: "smoothstep",
    markerEnd: { type: MarkerType.ArrowClosed, width: 16, height: 16 },
  }))
}
