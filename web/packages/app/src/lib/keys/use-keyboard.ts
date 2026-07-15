import { useEffect, useRef, useState } from "react"
import { useLocation, useNavigate } from "react-router-dom"

import { focusedId, rowCursor } from "@/lib/row-cursor"

import { bindings } from "./bindings"
import { Dispatcher } from "./dispatcher"
import type { RouteScope, RunContext } from "./types"

function routeScope(pathname: string): RouteScope {
  return pathname.startsWith("/task/") ? "detail" : "list"
}

export interface Keyboard {
  readonly overlayOpen: boolean
  readonly closeOverlay: () => void
}

export function useKeyboard(): Keyboard {
  const navigate = useNavigate()
  const location = useLocation()
  const [overlayOpen, setOverlayOpen] = useState(false)

  const live = useRef({ navigate, scope: routeScope(location.pathname), overlayOpen })
  live.current = { navigate, scope: routeScope(location.pathname), overlayOpen }

  useEffect(() => {
    const context = (): RunContext => ({
      navigate: (to) => live.current.navigate(to),
      overlay: {
        open: () => setOverlayOpen(true),
        close: () => setOverlayOpen(false),
        toggle: () => setOverlayOpen((open) => !open),
      },
      cursor: {
        moveBy: rowCursor.moveBy,
        focusedId: () => focusedId(rowCursor.getSnapshot()),
      },
    })
    const dispatcher = new Dispatcher({
      bindings,
      routeScope: () => live.current.scope,
      overlayOpen: () => live.current.overlayOpen,
      context,
    })
    return dispatcher.attach()
  }, [])

  return { overlayOpen, closeOverlay: () => setOverlayOpen(false) }
}
