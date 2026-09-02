import { useEffect, useRef, useState } from "react"
import { useLocation, useNavigate } from "react-router-dom"

import { boardPath, FLOW_ROUTE, taskRouteOf } from "@open-planner/task-ui"

import { copyTaskId } from "../clipboard"
import { detailActions, escapeOutcome } from "../detail-actions"
import { taskFlowPath } from "../flow-selection"
import { detailCursor, focusedRow, liveCursor } from "../row-cursor"
import { hoveredRow, taskAtHand } from "../row-target"
import { bindings } from "./bindings"
import { Dispatcher } from "./dispatcher"
import { historyIndex } from "./history"
import type { OverlayName, PaletteTarget, RouteScope, RunContext } from "./types"

function routeScope(pathname: string): RouteScope {
  if (pathname === FLOW_ROUTE) return "flow"
  return taskRouteOf(pathname) === undefined ? "list" : "detail"
}

export interface Keyboard {
  readonly activeOverlay: OverlayName | null
  readonly paletteTarget: PaletteTarget
  readonly closeOverlay: () => void
}

export function useKeyboard(): Keyboard {
  const navigate = useNavigate()
  const location = useLocation()
  const [activeOverlay, setActiveOverlay] = useState<OverlayName | null>(null)
  const [paletteTarget, setPaletteTarget] = useState<PaletteTarget>("home")

  const pathname = location.pathname
  const scope = routeScope(pathname)
  const live = useRef({ navigate, pathname, scope, activeOverlay })
  live.current = { navigate, pathname, scope, activeOverlay }

  // Unmounting a hovered row fires no mouseleave, so without this a row hovered on the way out of a
  // route would stay the task at hand on the next one.
  useEffect(() => {
    hoveredRow.clear()
  }, [pathname])

  // How many entries Esc can pop before leaving the stack we arrived on. Read from the router's own
  // history index rather than counted from navigation types, which report Back and Forward
  // identically and so would unwind the count on a Forward.
  const entryIndex = useRef(historyIndex())

  useEffect(() => {
    const activeCursor = () => liveCursor(live.current.scope)
    const targetTask = () => taskAtHand(activeCursor().getSnapshot(), live.current.pathname)
    const canGoBack = () => historyIndex() > entryIndex.current
    const context = (): RunContext => ({
      navigate: (to) => live.current.navigate(to),
      back: () => (canGoBack() ? live.current.navigate(-1) : live.current.navigate("/")),
      overlay: (name) => ({
        open: () => setActiveOverlay(name),
        close: () => setActiveOverlay((open) => (open === name ? null : open)),
        toggle: () => setActiveOverlay((open) => (open === name ? null : name)),
      }),
      palette: {
        open: (target) => {
          setPaletteTarget(target)
          setActiveOverlay("palette")
        },
      },
      cursor: {
        // The mouse clears the keyboard cursor (the row lists do that on mousemove); moving the
        // cursor hands the selection back, so it resumes from the hovered row and drops the hover.
        moveBy: (delta) => {
          const cursor = activeCursor()
          const hovered = hoveredRow.place(cursor.getSnapshot().rows)
          hoveredRow.clear()
          cursor.moveBy(delta, hovered)
        },
        // The hovered row renders as the current one, so it answers for the cursor when there is
        // no selection.
        focusedRow: () => {
          const cursor = activeCursor().getSnapshot()
          return focusedRow(cursor) ?? hoveredRow.among(cursor.rows)
        },
      },
      task: {
        // The key alone is what a user pastes into a task file or a command; the project is the
        // route's business, not the clipboard's.
        copyId: () => {
          const task = targetTask()
          if (task !== undefined) copyTaskId(task.id)
        },
        showFlow: () => {
          const task = targetTask()
          if (task !== undefined) live.current.navigate(taskFlowPath(task.project, task.id))
        },
      },
      detail: {
        editParent: () => detailActions.emit("edit-parent"),
        addSubtask: () => detailActions.emit("add-subtask"),
        editTags: () => detailActions.emit("edit-tags"),
        goToParent: () => detailActions.emit("go-parent"),
        escape: () => {
          const outcome = escapeOutcome(detailCursor.getSnapshot().index >= 0, canGoBack())
          if (outcome === "clear-selection") detailCursor.clear()
          else if (outcome === "back") live.current.navigate(-1)
          else {
            // Nothing to go back to: leave for the board of the project the task belongs to, which
            // is the page this detail would have been opened from.
            const task = taskRouteOf(live.current.pathname)
            live.current.navigate(task === undefined ? "/" : boardPath(task.project))
          }
        },
      },
    })
    const dispatcher = new Dispatcher({
      bindings,
      routeScope: () => live.current.scope,
      activeOverlay: () => live.current.activeOverlay,
      context,
    })
    return dispatcher.attach()
  }, [])

  return { activeOverlay, paletteTarget, closeOverlay: () => setActiveOverlay(null) }
}
