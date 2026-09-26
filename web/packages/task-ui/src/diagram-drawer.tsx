import { createContext, useContext } from "react"

import type { Drawing } from "@openplan/api-client"

export type DiagramOutcome = { readonly drawing: Drawing } | { readonly error: string; readonly line?: number }

export type DrawDiagram = (source: string) => Promise<DiagramOutcome>

const nobody: DrawDiagram = () => Promise.resolve({ error: "No daemon draws diagrams here." })

// The app owns the connection to the daemon, so this package asks through whatever it provides.
export const DiagramDrawer = createContext<DrawDiagram>(nobody)

export const useDiagramDrawer = (): DrawDiagram => useContext(DiagramDrawer)
