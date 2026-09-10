import { type ReactNode, useEffect, useState } from "react"

import { cn } from "@openplan/ui"

import { type DiagramResult, drawDiagram, svgDataUrl } from "./diagram"

function useDiagram(source: string): DiagramResult | null {
  const [result, setResult] = useState<{ source: string; result: DiagramResult } | null>(null)
  useEffect(() => {
    let current = true
    void drawDiagram(source).then((drawn) => {
      if (current) setResult({ source, result: drawn })
    })
    return () => {
      current = false
    }
  }, [source])
  return result?.source === source ? result.result : null
}

// d2 draws labels at 16px, larger than the prose around them. Three quarters keeps a wide diagram
// readable; past that, the figure scrolls sideways as a code block does.
const SCALE = 0.75
const imageClass = "my-0 h-auto max-w-none"

// The canvas colours of d2 themes 3 and 200, so the frame and the picture read as one surface.
const canvasClass = "bg-white dark:bg-[#1e1e2e]"

// The light and dark renders both mount, and the app theme picks one in CSS, so a theme change
// never draws again. An <img> keeps the SVG inert: nothing inside it runs or reaches the page.
export function DiagramBlock({ source, children }: { source: string; children: ReactNode }) {
  const result = useDiagram(source)
  if (result === null) return children
  if ("error" in result) {
    return (
      <div className="my-3" data-diagram="error">
        <p role="alert" className="text-danger my-0 mb-1 font-mono text-xs whitespace-pre-wrap">
          {result.error}
        </p>
        {children}
      </div>
    )
  }
  const { light, dark, size } = result.svg
  const width = Math.round(size.width * SCALE)
  const height = Math.round(size.height * SCALE)
  return (
    <figure
      className={cn("border-border my-3 overflow-x-auto rounded-lg border p-2", canvasClass)}
      data-diagram="drawn"
    >
      <img
        src={svgDataUrl(light)}
        alt="Diagram"
        width={width}
        height={height}
        className={cn(imageClass, "dark:hidden")}
      />
      <img
        src={svgDataUrl(dark)}
        alt="Diagram"
        width={width}
        height={height}
        className={cn(imageClass, "hidden dark:block")}
      />
    </figure>
  )
}
