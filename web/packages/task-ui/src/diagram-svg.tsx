import type { Drawing } from "@openplan/api-client"

import { useDiagramFont } from "./diagram-font"

// The daemon's writer is the trust boundary: it escapes every label and writes no script, event
// attribute, or link from diagram source. So the SVG goes inline, where the theme CSS reaches it.
export function DiagramSvg({ drawing, className }: { drawing: Drawing; className?: string }) {
  const fontReady = useDiagramFont()
  if (!fontReady) return <div style={{ width: drawing.width, height: drawing.height }} className={className} />
  return <div className={className} dangerouslySetInnerHTML={{ __html: drawing.svg }} />
}
