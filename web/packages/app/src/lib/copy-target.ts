let hovered: string | undefined

export const hoveredRow = {
  enter: (id: string): void => {
    hovered = id
  },
  // A pointer crossing a row boundary enters the new row before it leaves the old one, so a leave
  // naming a row the pointer has already moved past must not clear the newer hover.
  leave: (id: string): void => {
    if (hovered === id) hovered = undefined
  },
  clear: (): void => {
    hovered = undefined
  },
  current: (): string | undefined => hovered,
}

export function copyTargetId(
  hoveredId: string | undefined,
  focusedId: string | undefined,
  routeId: string | undefined,
): string | undefined {
  return hoveredId ?? focusedId ?? routeId
}

const TASK_ROUTE = /^\/task\/([^/]+)/

export function routeTaskId(pathname: string): string | undefined {
  return TASK_ROUTE.exec(pathname)?.[1]
}
