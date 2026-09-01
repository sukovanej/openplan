import { useQuery, type UseQueryResult } from "@tanstack/react-query"
import {
  Background,
  BackgroundVariant,
  Controls,
  type Edge,
  type EdgeTypes,
  MarkerType,
  type Node,
  type NodeTypes,
  ReactFlow,
  useReactFlow,
} from "@xyflow/react"
import { useEffect, useMemo, useRef, useState } from "react"
import { Link, useSearchParams } from "react-router-dom"

import type { Flow } from "@open-planner/api-client"
import { FLOW_ROUTE } from "@open-planner/task-ui"
import { EmptyState, Panel, PanelBody, PanelHeader, PanelTitle, Skeleton } from "@open-planner/ui"

import { BoxFlowCard, LeafFlowCard, RoutedFlowLine, UnresolvedFlowCard } from "../components/flow-nodes"
import { FlowCycles, getFlow } from "../lib/api"
import { type FlowLayout, layoutFlow } from "../lib/flow-layout"
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
}

const EDGE_TYPES: EdgeTypes = { routed: RoutedFlowLine }

// A card stops being readable below this, so the graph never shrinks past it. Whatever is left over
// the reader pans to.
const MIN_ZOOM = 0.5

export function FlowRoute() {
  const [params] = useSearchParams()
  const query = params.toString()
  const selection = useMemo(() => readSelection(new URLSearchParams(query)), [query])
  const flow = useQuery({
    queryKey: flowKey(selectionParams(selection).toString()),
    queryFn: () => runtime.runPromise(getFlow(selection)),
  })
  const page = useRef<HTMLDivElement>(null)
  const shape = usePageShape(page)
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
      <PanelBody ref={page} className="overflow-hidden">
        <FlowState flow={flow} shape={shape} />
      </PanelBody>
    </Panel>
  )
}

// The islands are packed to the shape of the box they are drawn in, so the whole graph fills it
// rather than running off one edge.
function usePageShape(page: React.RefObject<HTMLDivElement | null>): number {
  const [shape, setShape] = useState(1.8)
  useEffect(() => {
    const box = page.current
    if (box === null) return
    const watch = new ResizeObserver(() => {
      const { width, height } = box.getBoundingClientRect()
      if (width > 0 && height > 0) setShape(Math.round((width / height) * 10) / 10)
    })
    watch.observe(box)
    return () => watch.disconnect()
  }, [page])
  return shape
}

function FlowState({ flow, shape }: { flow: UseQueryResult<Flow>; shape: number }) {
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
  return <Diagram flow={flow.data} shape={shape} />
}

function round(cycle: ReadonlyArray<string>): string {
  return [...cycle, cycle[0]].join(" → ")
}

function Diagram({ flow, shape }: { flow: Flow; shape: number }) {
  const { resolved } = useTheme()
  const [layout, setLayout] = useState<FlowLayout>()

  // The old drawing stays up while the engine runs, so a refetch does not blank the page.
  useEffect(() => {
    let live = true
    void layoutFlow(flow, shape).then((next) => {
      if (live) setLayout(next)
    })
    return () => {
      live = false
    }
  }, [flow, shape])

  const nodes = useMemo(() => (layout === undefined ? [] : reactFlowNodes(layout)), [layout])
  const edges = useMemo(() => (layout === undefined ? [] : reactFlowEdges(layout)), [layout])
  if (layout === undefined) return <Skeleton className="m-6 h-[calc(100%-3rem)]" />
  return (
    <ReactFlow
      nodes={nodes}
      edges={edges}
      nodeTypes={NODE_TYPES}
      edgeTypes={EDGE_TYPES}
      colorMode={resolved}
      fitView
      fitViewOptions={{ maxZoom: 1, padding: 0.06 }}
      minZoom={MIN_ZOOM}
      maxZoom={1.6}
      // The layout comes from the endpoint, so a node holds still and answers the click that opens
      // its task. React Flow gives a node pointer events only while it is selectable.
      nodesDraggable={false}
      nodesConnectable={false}
      deleteKeyCode={null}
      className="h-full w-full"
    >
      <Refit on={shape} />
      <Background variant={BackgroundVariant.Dots} gap={24} size={1} />
      <Controls showInteractive={false} />
    </ReactFlow>
  )
}

// A resize reshapes the packing, which leaves the old viewport pointing at nothing.
function Refit({ on }: { on: number }) {
  const flow = useReactFlow()
  const first = useRef(true)
  useEffect(() => {
    if (first.current) {
      first.current = false
      return
    }
    void flow.fitView({ maxZoom: 1, padding: 0.06 })
  }, [on, flow])
  return null
}

function reactFlowNodes(layout: FlowLayout): Array<Node> {
  return layout.nodes.map((node) => ({
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
}

function reactFlowEdges(layout: FlowLayout): Array<Edge> {
  return layout.edges.map((edge) => ({
    id: edge.key,
    source: edge.source,
    target: edge.target,
    type: "routed",
    data: { points: edge.points },
    markerEnd: { type: MarkerType.ArrowClosed, width: 16, height: 16 },
    // An edge that leaves a box has to paint over it.
    zIndex: 1000,
  }))
}
