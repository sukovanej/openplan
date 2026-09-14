import { X } from "lucide-react"
import { type ReactNode, useEffect, useState } from "react"

import { Button, cn, Modal, Skeleton } from "@openplan/ui"

import { type DiagramResult, type DiagramTheme, drawDiagram, type DrawnDiagram, svgDataUrl } from "./diagram"
import { useResolvedTheme } from "./use-dark-mode"

type Drawing = { source: string; theme: DiagramTheme; result: DiagramResult }

// The last picture stays up while the other theme's render is on its way, so a theme flip never
// drops back to the placeholder.
function useDiagram(source: string): { current: Drawing | null; last: DrawnDiagram | null } {
  const theme = useResolvedTheme()
  const [drawing, setDrawing] = useState<Drawing | null>(null)
  useEffect(() => {
    let current = true
    void drawDiagram(source, theme).then((result) => {
      if (current) setDrawing({ source, theme, result })
    })
    return () => {
      current = false
    }
  }, [source, theme])
  const matches = drawing !== null && drawing.source === source
  const current = matches && drawing.theme === theme ? drawing : null
  const last = matches && "drawn" in drawing.result ? drawing.result.drawn : null
  return { current, last }
}

// d2 draws labels at 16px, larger than the prose around them. A diagram wider than the column
// shrinks further to fit it.
const SCALE = 0.75

// The canvas colours of d2 themes 3 and 200, so the frame and the picture read as one surface.
const canvasColors = "bg-white dark:bg-[#1e1e2e]"
const frameClass = cn("border-border my-3 rounded-lg border p-2", canvasColors)

function Picture({ drawn }: { drawn: DrawnDiagram }) {
  return (
    <img
      src={svgDataUrl(drawn.svg)}
      alt="Diagram"
      width={Math.round(drawn.size.width * SCALE)}
      height={Math.round(drawn.size.height * SCALE)}
      className="mx-auto my-0 block h-auto max-w-full"
    />
  )
}

// The full view holds the focus, and `data-keys-ignore` keeps the page's single-key bindings off it
// while it is open.
function DrawnFigure({ drawn }: { drawn: DrawnDiagram }) {
  const [fullView, setFullView] = useState(false)
  const close = () => setFullView(false)
  return (
    <>
      <figure className={frameClass} data-diagram="drawn">
        <button
          type="button"
          aria-label="Show the diagram in full view"
          onClick={() => setFullView(true)}
          className="block w-full cursor-zoom-in focus:outline-none"
        >
          <Picture drawn={drawn} />
        </button>
      </figure>
      {/* The modal centres its content in a padded backdrop. The full view covers the whole window
          instead, so the picture gets all of it. */}
      <Modal
        open={fullView}
        onClose={close}
        label="Diagram"
        className={cn("fixed inset-0 p-4", canvasColors)}
        data-keys-ignore
      >
        <img src={svgDataUrl(drawn.svg)} alt="Diagram" className="block size-full object-contain" />
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

// An <img> keeps the SVG inert: nothing inside it runs or reaches the page.
export function DiagramBlock({ source, children }: { source: string; children: ReactNode }) {
  const { current, last } = useDiagram(source)
  if (current === null && last === null) {
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
  if (current !== null && "error" in current.result) {
    return (
      <div className="my-3" data-diagram="error">
        <p role="alert" className="text-danger my-0 mb-1 font-mono text-xs whitespace-pre-wrap">
          {current.result.error}
        </p>
        {children}
      </div>
    )
  }
  const drawn = current !== null && "drawn" in current.result ? current.result.drawn : last!
  return <DrawnFigure drawn={drawn} />
}
