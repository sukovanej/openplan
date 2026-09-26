import { keepPreviousData, useQuery, type UseQueryResult } from "@tanstack/react-query"
import { useLayoutEffect, useMemo, useRef, useState } from "react"
import { Link, useSearchParams } from "react-router-dom"

import type { Drawing } from "@openplan/api-client"
import { DiagramViewport, FLOW_ROUTE } from "@openplan/task-ui"
import { EmptyState, Panel, PanelBody, PanelHeader, PanelTitle, Skeleton } from "@openplan/ui"

import { drawFlow, FlowCycles, type PageSize } from "../lib/api"
import { describeSelection, readSelection, selectionParams, selectsEveryTask } from "../lib/flow-selection"
import { errorText } from "../lib/format"
import { flowKey } from "../lib/query-client"
import { useRowCursor } from "../lib/row-cursor"
import { abortable } from "../lib/runtime"

// This page holds no task rows, and the cursor is the board's — left as it was, `j` then Enter here
// would open a task the reader can no longer see.
const NO_ROWS: ReadonlyArray<string> = []

// A card stops being readable below this, so the first view never shrinks past it. Whatever is left
// over the reader pans to.
const FIT_FLOOR = 0.5

export function FlowRoute() {
  const [params] = useSearchParams()
  const query = params.toString()
  const selection = useMemo(() => readSelection(new URLSearchParams(query)), [query])
  const page = useRef<HTMLDivElement>(null)
  const size = usePageSize(page)
  const key = flowKey(selectionParams(selection).toString(), size?.width ?? 0, size?.height ?? 0)
  const flow = useQuery({
    queryKey: key,
    queryFn: async (context): Promise<Drawn> => ({
      drawing: await abortable(drawFlow(selection, size!))(context),
      layout: key.join(" "),
    }),
    enabled: size !== undefined,
    // The old drawing stays up while the daemon packs the flow to a new page shape.
    placeholderData: keepPreviousData,
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
      <PanelBody ref={page} className="overflow-hidden">
        <FlowState flow={flow} />
      </PanelBody>
    </Panel>
  )
}

// A drag of the window edge settles first, so the daemon packs the flow once for the size the
// reader stops at.
const SETTLE_MS = 200

// The daemon packs the parts of the flow to the shape of the page, so the first size is read before
// anything is asked for: a guess would draw the flow once and then move every part.
function usePageSize(page: React.RefObject<HTMLDivElement | null>): PageSize | undefined {
  const [size, setSize] = useState<PageSize>()
  useLayoutEffect(() => {
    const element = page.current
    if (element === null) return
    const read = () => {
      const box = element.getBoundingClientRect()
      const width = Math.round(box.width)
      const height = Math.round(box.height)
      if (width > 0 && height > 0) {
        setSize((held) => (held?.width === width && held.height === height ? held : { width, height }))
      }
    }
    read()
    let settle: ReturnType<typeof setTimeout> | undefined
    const watch = new ResizeObserver(() => {
      clearTimeout(settle)
      settle = setTimeout(read, SETTLE_MS)
    })
    watch.observe(element)
    return () => {
      clearTimeout(settle)
      watch.disconnect()
    }
  }, [page])
  return size
}

// `layout` names the selection and the page the drawing answers, so a new one fits the view again
// and a redraw after a task changes keeps the reader's place.
interface Drawn {
  readonly drawing: Drawing
  readonly layout: string
}

function FlowState({ flow }: { flow: UseQueryResult<Drawn> }) {
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
  if (flow.data === undefined) return <Skeleton className="m-6 h-[calc(100%-3rem)]" />
  const { drawing, layout } = flow.data
  if (drawing.width === 0) {
    return (
      <div className="p-6">
        <EmptyState title="Nothing to order" detail="No task matches this flow." />
      </div>
    )
  }
  return <DiagramViewport drawing={drawing} label="Flow" fitKey={layout} fitFloor={FIT_FLOOR} />
}

function round(cycle: ReadonlyArray<string>): string {
  return [...cycle, cycle[0]].join(" → ")
}
