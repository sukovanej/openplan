import { useSyncExternalStore } from "react"

// The daemon lays a diagram out with the glyph widths of this face alone. The face loads only when
// text first asks for it, and a diagram painted before that shows a fallback font whose labels
// spill out of their boxes.
const FACES = ['400 14px "Inter Diagram"', '600 14px "Inter Diagram"']

let ready = false
let loading: Promise<void> | undefined
const listeners = new Set<() => void>()

// A face that fails to load leaves the fallback, and a diagram in it beats no diagram. A document
// with no font set (a test DOM) has nothing to wait for.
function load(): void {
  const fonts = (document as { fonts?: FontFaceSet }).fonts
  loading ??= Promise.all(FACES.map((face) => fonts?.load(face)))
    .catch(() => undefined)
    .then(() => {
      ready = true
      for (const listener of listeners) listener()
    })
}

export function useDiagramFont(): boolean {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener)
      load()
      return () => listeners.delete(listener)
    },
    () => ready,
    () => false,
  )
}
