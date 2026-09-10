import { Loader2 } from "lucide-react"
import { type ReactNode, useEffect, useState } from "react"

import { cn } from "@openplan/ui"

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

// d2 draws labels at 16px, larger than the prose around them. Three quarters keeps a wide diagram
// readable; past that, the figure scrolls sideways as a code block does.
const SCALE = 0.75

// The canvas colours of d2 themes 3 and 200, so the frame and the picture read as one surface.
const frameClass = "border-border my-3 overflow-x-auto rounded-lg border bg-white p-2 dark:bg-[#1e1e2e]"

function Picture({ drawn }: { drawn: DrawnDiagram }) {
  return (
    <img
      src={svgDataUrl(drawn.svg)}
      alt="Diagram"
      width={Math.round(drawn.size.width * SCALE)}
      height={Math.round(drawn.size.height * SCALE)}
      className="my-0 h-auto max-w-none"
    />
  )
}

// An <img> keeps the SVG inert: nothing inside it runs or reaches the page.
export function DiagramBlock({ source, children }: { source: string; children: ReactNode }) {
  const { current, last } = useDiagram(source)
  if (current === null && last === null) {
    return (
      <figure className={cn(frameClass, "flex min-h-32 items-center justify-center")} data-diagram="drawing">
        <p role="status" className="text-muted-foreground my-0 flex items-center gap-2 font-sans text-sm">
          <Loader2 className="size-4 animate-spin" />
          Drawing the diagram
        </p>
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
  return (
    <figure className={frameClass} data-diagram="drawn">
      <Picture drawn={drawn} />
    </figure>
  )
}
