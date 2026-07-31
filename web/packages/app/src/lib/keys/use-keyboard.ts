import { useEffect, useRef, useState } from "react"
import { useLocation, useNavigate } from "react-router-dom"

import { TASK_ROUTE, taskIdOf } from "@open-planner/task-ui"

import { copyTaskId } from "../clipboard"
import { copyTargetId, hoveredRow } from "../copy-target"
import { detailActions, escapeOutcome } from "../detail-actions"
import { focusedId, rowCursor, subtaskCursor } from "../row-cursor"
import { bindings } from "./bindings"
import { Dispatcher } from "./dispatcher"
import { historyIndex } from "./history"
import type { RouteScope, RunContext } from "./types"

function routeScope(pathname: string): RouteScope {
  return pathname.startsWith(TASK_ROUTE) ? "detail" : "list"
}

export interface Keyboard {
  readonly overlayOpen: boolean
  readonly closeOverlay: () => void
}

export function useKeyboard(): Keyboard {
  const navigate = useNavigate()
  const location = useLocation()
  const [overlayOpen, setOverlayOpen] = useState(false)

  const pathname = location.pathname
  const scope = routeScope(pathname)
  const live = useRef({ navigate, pathname, scope, overlayOpen })
  live.current = { navigate, pathname, scope, overlayOpen }

  // Unmounting a hovered row fires no mouseleave, so without this a row hovered on the way out of a
  // route would stay the copy target on the next one.
  useEffect(() => {
    hoveredRow.clear()
  }, [pathname])

  // How many entries Esc can pop before leaving the stack we arrived on. Read from the router's own
  // history index rather than counted from navigation types, which report Back and Forward
  // identically and so would unwind the count on a Forward.
  const entryIndex = useRef(historyIndex())

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
        // The mouse clears the keyboard cursor (the row lists do that on mousemove); moving the
        // cursor hands the selection back, so it resumes from the hovered row and drops the hover.
        moveBy: (delta) => {
          const cursor = activeCursor()
          const hovered = hoveredRow.among(cursor.getSnapshot().ids)
          hoveredRow.clear()
          cursor.moveBy(delta, hovered)
        },
        // The hovered row renders as the current one, so it answers for the cursor when there is
        // no selection.
        focusedId: () => {
          const cursor = activeCursor().getSnapshot()
          return focusedId(cursor) ?? hoveredRow.among(cursor.ids)
        },
      },
      copy: {
        taskId: () => {
          const cursor = activeCursor().getSnapshot()
          const id = copyTargetId(hoveredRow.among(cursor.ids), focusedId(cursor), taskIdOf(live.current.pathname))
          if (id !== undefined) copyTaskId(id)
        },
      },
      detail: {
        editParent: () => detailActions.emit("edit-parent"),
        addSubtask: () => detailActions.emit("add-subtask"),
        goToParent: () => detailActions.emit("go-parent"),
        escape: () => {
          const canGoBack = historyIndex() > entryIndex.current
          const outcome = escapeOutcome(subtaskCursor.getSnapshot().index >= 0, canGoBack)
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
