import { Maximize, Minus, Plus } from "lucide-react"
import { type MouseEvent, useEffectEvent, useLayoutEffect, useRef } from "react"
import { useNavigate } from "react-router-dom"

import type { Drawing } from "@openplan/api-client"
import { Button, cn } from "@openplan/ui"

import { DiagramSvg } from "./diagram-svg"
import { type PanZoom, panZoom } from "./pan-zoom"

const BUTTON_STEP = 1.25

// A link that starts with one `/` is a page of this app. The router opens it in place; a reload
// would drop every read the app holds. A modified click still opens a new tab.
function followInApp(event: MouseEvent, navigate: (path: string) => void): void {
  if (event.defaultPrevented || event.button !== 0) return
  if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return
  const link = (event.target as Element).closest("a")
  const href = link?.getAttribute("href")
  if (href == null || !href.startsWith("/") || href.startsWith("//")) return
  event.preventDefault()
  navigate(href)
}

// `fitKey` names the layout the fit belongs to: a new key fits the drawing again, and a new
// drawing under the same key keeps the reader's pan and zoom.
export function DiagramViewport({
  drawing,
  label,
  fitKey,
  fitFloor,
  className,
}: {
  drawing: Drawing
  label: string
  fitKey?: string
  fitFloor?: number
  className?: string
}) {
  const frame = useRef<HTMLDivElement>(null)
  const layer = useRef<HTMLDivElement>(null)
  const control = useRef<PanZoom>(null)
  const navigate = useNavigate()

  // A layout effect, so the first fit below finds it and the drawing never paints unfitted.
  useLayoutEffect(() => {
    const view = panZoom(frame.current!, layer.current!)
    control.current = view
    return () => {
      view.dispose()
      control.current = null
    }
  }, [])

  const { width, height } = drawing
  const fit = () => control.current?.fit({ width, height }, fitFloor)
  const refit = useEffectEvent(fit)
  useLayoutEffect(() => refit(), [fitKey])

  return (
    <div
      ref={frame}
      role="group"
      aria-label={label}
      onClick={(event) => followInApp(event, navigate)}
      className={cn(
        "relative size-full cursor-grab touch-none overflow-hidden select-none data-[panning]:cursor-grabbing",
        className,
      )}
    >
      <div ref={layer} className="absolute top-0 left-0 origin-top-left">
        <DiagramSvg drawing={drawing} />
      </div>
      <div className="bg-background/90 absolute right-3 bottom-3 flex flex-col rounded-md border shadow-sm">
        <Button size="icon" aria-label="Zoom in" onClick={() => control.current?.zoomBy(BUTTON_STEP)}>
          <Plus className="size-4" aria-hidden="true" />
        </Button>
        <Button size="icon" aria-label="Zoom out" onClick={() => control.current?.zoomBy(1 / BUTTON_STEP)}>
          <Minus className="size-4" aria-hidden="true" />
        </Button>
        <Button size="icon" aria-label="Fit to the page" onClick={fit}>
          <Maximize className="size-4" aria-hidden="true" />
        </Button>
      </div>
    </div>
  )
}
