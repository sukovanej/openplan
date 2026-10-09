import { useCallback, useEffect, useEffectEvent, useRef, useState } from "react"
import { useLocation, useNavigate } from "react-router-dom"

import {
  boardPath,
  docRouteOf,
  docsPath,
  FLOW_ROUTE,
  isActivityPath,
  projectOfPath,
  taskRouteOf,
} from "@openplan/task-ui"

import { copyTaskId } from "../clipboard"
import { detailActions, escapeOutcome } from "../detail-actions"
import { taskFlowPath } from "../flow-selection"
import { openProjectMenu } from "../project-menu"
import { pagePath, selectedProject, selectedProjects, selects, switchProjectPath } from "../project-scope"
import { useProjects } from "../projects"
import { detailCursor, focusedRow, liveCursor } from "../row-cursor"
import { hoveredRow, taskAtHand } from "../row-target"
import { statusRequests } from "../status-requests"
import { bindings } from "./bindings"
import { Dispatcher } from "./dispatcher"
import { historyIndex } from "./history"
import type { OverlayName, PaletteTarget, RouteScope, RunContext } from "./types"

function routeScope(pathname: string): RouteScope {
  if (pathname === FLOW_ROUTE) return "flow"
  if (isActivityPath(pathname)) return "activity"
  if (docRouteOf(pathname) !== undefined) return "detail"
  return taskRouteOf(pathname) === undefined ? "list" : "detail"
}

// A task and the activity belong to a board, and a doc belongs to the docs list: the page each would
// have been opened from.
function pageAbove(pathname: string): string {
  const doc = docRouteOf(pathname)
  if (doc !== undefined) return docsPath(doc.project)
  return boardPath(projectOfPath(pathname))
}

export interface Keyboard {
  readonly activeOverlay: OverlayName | null
  readonly paletteTarget: PaletteTarget
  readonly openPalette: (target: PaletteTarget) => void
  readonly openNewTask: () => void
  readonly closeOverlay: (name: OverlayName) => void
}

export function useKeyboard(): Keyboard {
  const navigate = useNavigate()
  const location = useLocation()
  const [activeOverlay, setActiveOverlay] = useState<OverlayName | null>(null)
  const [paletteTarget, setPaletteTarget] = useState<PaletteTarget>("home")

  const { pathname, search } = location
  const scope = routeScope(pathname)
  const projects = useProjects()
  const openPalette = useCallback((target: PaletteTarget) => {
    setPaletteTarget(target)
    setActiveOverlay("palette")
  }, [])
  // Another overlay can ask for this one as it closes, so a close leaves the overlay it does not name.
  const closeOverlay = useCallback((name: OverlayName) => setActiveOverlay((open) => (open === name ? null : open)), [])
  const openNewTask = useCallback(() => setActiveOverlay("new-task"), [])
  const live = useEffectEvent(() => ({ navigate, pathname, search, scope, activeOverlay, projects }))

  // Unmounting a hovered row fires no mouseleave, so without this a row hovered on the way out of a
  // route would stay the task at hand on the next one.
  useEffect(() => {
    hoveredRow.clear()
  }, [pathname])

  // A link that navigated here keeps the focus, and Enter belongs to a focused control — so Enter on
  // the page it opened would walk back through that link instead of opening the row at hand. The
  // link did its work; the new page is not its page.
  useEffect(() => {
    const held = document.activeElement
    if (held instanceof HTMLAnchorElement) held.blur()
  }, [pathname])

  // How many entries Esc can pop before leaving the stack we arrived on. Read from the router's own
  // history index rather than counted from navigation types, which report Back and Forward
  // identically and so would unwind the count on a Forward.
  const entryIndex = useRef(historyIndex())

  useEffect(() => {
    const activeCursor = () => liveCursor(live().scope)
    const targetTask = () => taskAtHand(activeCursor().getSnapshot(), live().pathname)
    const canGoBack = () => historyIndex() > entryIndex.current
    const context = (): RunContext => ({
      navigate: (to) => live().navigate(to),
      goToPage: (page) => live().navigate(pagePath(page, selectedProject(live().pathname, live().search))),
      chooseProject: openProjectMenu,
      selectProject: (digit) => {
        const { projects, pathname, search } = live()
        if (projects === undefined || digit > projects.length) return
        const project = digit === 0 ? undefined : projects[digit - 1].name
        if (!selects(selectedProjects(pathname, search), project)) {
          live().navigate(switchProjectPath(pathname, search, project))
        }
      },
      back: () => (canGoBack() ? live().navigate(-1) : live().navigate(pageAbove(live().pathname))),
      overlay: (name) => ({
        open: () => setActiveOverlay(name),
        close: () => closeOverlay(name),
        toggle: () => setActiveOverlay((open) => (open === name ? null : name)),
      }),
      palette: { open: openPalette },
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
          if (task !== undefined) live().navigate(taskFlowPath(task.project, task.id))
        },
        // The row at hand opens its own menu, which is where the mark it changes is drawn.
        editStatus: () => {
          const task = targetTask()
          if (task !== undefined) statusRequests.emit(task)
        },
      },
      detail: {
        editParent: () => detailActions.emit("edit-parent"),
        addSubtask: () => detailActions.emit("add-subtask"),
        editTags: () => detailActions.emit("edit-tags"),
        editDescription: () => detailActions.emit("edit-description"),
        goToParent: () => detailActions.emit("go-parent"),
        escape: () => {
          const outcome = escapeOutcome(detailCursor.getSnapshot().index >= 0, canGoBack())
          if (outcome === "clear-selection") detailCursor.clear()
          else if (outcome === "back") live().navigate(-1)
          else live().navigate(pageAbove(live().pathname))
        },
      },
    })
    const dispatcher = new Dispatcher({
      bindings,
      routeScope: () => live().scope,
      activeOverlay: () => live().activeOverlay,
      context,
    })
    return dispatcher.attach()
  }, [openPalette, closeOverlay])

  return { activeOverlay, paletteTarget, openPalette, openNewTask, closeOverlay }
}
