import { X } from "lucide-react"
import { useEffect, useState } from "react"

import type { Drawing } from "@openplan/api-client"
import { Button, cn, Modal, Skeleton } from "@openplan/ui"

import { type DiagramOutcome, useDiagramDrawer } from "./diagram-drawer"
import { DiagramSvg } from "./diagram-svg"
import { DiagramViewport } from "./diagram-viewport"

function useDiagram(source: string): DiagramOutcome | null {
  const draw = useDiagramDrawer()
  const [outcome, setOutcome] = useState<{ source: string; outcome: DiagramOutcome } | null>(null)
  useEffect(() => {
    let current = true
    void draw(source).then((drawn) => {
      if (current) setOutcome({ source, outcome: drawn })
    })
    return () => {
      current = false
    }
  }, [draw, source])
  return outcome?.source === source ? outcome.outcome : null
}

const frameClass = "my-3"

// The full view holds the focus, and `data-keys-ignore` keeps the page's single-key bindings off it
// while it is open.
function DrawnFigure({ drawing }: { drawing: Drawing }) {
  const [fullView, setFullView] = useState(false)
  const close = () => setFullView(false)
  return (
    <>
      <figure className={frameClass} data-diagram="drawn">
        <button
          type="button"
          aria-label="Show the diagram in full view"
          onClick={() => setFullView(true)}
          className="block w-full cursor-zoom-in overflow-hidden focus:outline-none"
        >
          <DiagramSvg
            drawing={drawing}
            className="mx-auto w-fit max-w-full [&_svg]:block [&_svg]:h-auto [&_svg]:max-w-full"
          />
        </button>
      </figure>
      {/* The modal centres its content in a padded backdrop. The full view covers the whole window
          instead, so the drawing gets all of it. */}
      <Modal open={fullView} onClose={close} label="Diagram" className="bg-background fixed inset-0" data-keys-ignore>
        <DiagramViewport drawing={drawing} label="Diagram" />
        <Button
          size="icon"
          aria-label="Close"
          onClick={close}
          className="bg-background/80 text-foreground absolute top-3 right-3 border shadow-sm"
        >
          <X className="size-4" aria-hidden="true" />
        </Button>
      </Modal>
    </>
  )
}

function Refusal({ source, error, line }: { source: string; error: string; line?: number }) {
  return (
    <div className="my-3" data-diagram="error">
      <p role="alert" className="text-danger my-0 mb-1 font-mono text-xs whitespace-pre-wrap">
        {error}
      </p>
      <pre>
        <code>
          {source.split("\n").map((text, index) => (
            <span
              key={index}
              data-failed={index + 1 === line ? "" : undefined}
              className="data-[failed]:bg-danger-surface data-[failed]:text-danger-foreground block"
            >
              {text === "" ? "​" : text}
            </span>
          ))}
        </code>
      </pre>
    </div>
  )
}

export function DiagramBlock({ source }: { source: string }) {
  const outcome = useDiagram(source)
  if (outcome === null) {
    return (
      <figure className={frameClass} data-diagram="drawing" role="status">
        <span className="sr-only">Drawing the diagram</span>
        <div className="flex flex-col items-center gap-4 py-4">
          <Skeleton className="h-10 w-28" />
          <Skeleton className="h-8 w-0.5" />
          <div className="flex gap-6">
            <Skeleton className="h-10 w-28" />
            <Skeleton className="h-10 w-28" />
          </div>
        </div>
      </figure>
    )
  }
  if ("error" in outcome) {
    return <Refusal source={source.replace(/\n$/, "")} error={outcome.error} line={outcome.line} />
  }
  if (outcome.drawing.width === 0) {
    return (
      <figure className={cn(frameClass, "text-muted-foreground text-center text-xs")} data-diagram="empty">
        The diagram is empty.
      </figure>
    )
  }
  return <DrawnFigure drawing={outcome.drawing} />
}
