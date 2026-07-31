import { TASK_ROUTE } from "@open-planner/task-ui"

let hovered: string | undefined

export const hoveredRow = {
  enter: (id: string): void => {
    hovered = id
  },
  // Guards against a leave that names a row the store has already moved past, whatever order the
  // leaving and entering rows report in.
  leave: (id: string): void => {
    if (hovered === id) hovered = undefined
  },
  clear: (): void => {
    hovered = undefined
  },
  // A row that goes away under a still pointer — deleted, reparented, refreshed out of the list —
  // fires no mouseleave, so a hover counts only while its row is one of those on screen.
  among: (rendered: ReadonlyArray<string>): string | undefined =>
    hovered !== undefined && rendered.includes(hovered) ? hovered : undefined,
}

export function copyTargetId(
  hoveredId: string | undefined,
  focusedId: string | undefined,
  routeId: string | undefined,
): string | undefined {
  return hoveredId ?? focusedId ?? routeId
}

const TASK_ID = new RegExp(`^${TASK_ROUTE}([^/]+)`)

export function routeTaskId(pathname: string): string | undefined {
  return TASK_ID.exec(pathname)?.[1]
}
