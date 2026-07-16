import { useEffect, useRef, useState } from "react"
import { useLocation, useNavigate, useNavigationType } from "react-router-dom"

import { detailActions, escapeOutcome } from "@/lib/detail-actions"
import { focusedId, rowCursor, subtaskCursor } from "@/lib/row-cursor"

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
  const navigationType = useNavigationType()
  const [overlayOpen, setOverlayOpen] = useState(false)

  const scope = routeScope(location.pathname)
  const live = useRef({ navigate, scope, overlayOpen })
  live.current = { navigate, scope, overlayOpen }

  // How many in-app entries can Esc pop before there is nothing of ours left; PUSH deepens it, POP
  // unwinds it, and a branch-only REPLACE leaves it be. At zero, Esc falls back to the list.
  const depth = useRef(0)
  useEffect(() => {
    if (navigationType === "PUSH") depth.current += 1
    else if (navigationType === "POP") depth.current = Math.max(0, depth.current - 1)
  }, [location.key, navigationType])

  useEffect(() => {
    const activeCursor = () => (live.current.scope === "detail" ? subtaskCursor : rowCursor)
    const context = (): RunContext => ({
      navigate: (to) => live.current.navigate(to),
      overlay: {
        open: () => setOverlayOpen(true),
        close: () => setOverlayOpen(false),
        toggle: () => setOverlayOpen((open) => !open),
      },
      cursor: {
        moveBy: (delta) => activeCursor().moveBy(delta),
        focusedId: () => focusedId(activeCursor().getSnapshot()),
      },
      detail: {
        editParent: () => detailActions.emit("edit-parent"),
        addSubtask: () => detailActions.emit("add-subtask"),
        goToParent: () => detailActions.emit("go-parent"),
        escape: () => {
          const outcome = escapeOutcome(subtaskCursor.getSnapshot().index >= 0, depth.current > 0)
          if (outcome === "clear-selection") subtaskCursor.clear()
          else if (outcome === "back") live.current.navigate(-1)
          else live.current.navigate("/")
        },
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
