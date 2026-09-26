import { createContext, useContext } from "react"

import type { Drawing } from "@openplan/api-client"

// `error` is the daemon's refusal of the source. `failed` is a draw that never reached a verdict, such
// as a daemon that did not answer, so the same source can draw on a second try.
export type DiagramOutcome =
  | { readonly drawing: Drawing }
  | { readonly error: string; readonly line?: number }
  | { readonly failed: string }

export type DrawDiagram = (source: string) => Promise<DiagramOutcome>

const nobody: DrawDiagram = () => Promise.resolve({ failed: "No daemon draws diagrams here." })

// The app owns the connection to the daemon, so this package asks through whatever it provides.
export const DiagramDrawer = createContext<DrawDiagram>(nobody)

export const useDiagramDrawer = (): DrawDiagram => useContext(DiagramDrawer)
