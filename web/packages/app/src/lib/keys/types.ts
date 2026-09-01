// Every overlay the app can put over the page. One is open at a time, and each is its own key
// scope, so two overlays can bind the same key without either one firing under the other.
export const OVERLAY_NAMES = ["help", "palette"] as const
export type OverlayName = (typeof OVERLAY_NAMES)[number]

export type Scope = "global" | "list" | "detail" | "rows" | OverlayName
export type RouteScope = "list" | "detail"

export function isOverlayScope(scope: Scope): scope is OverlayName {
  return (OVERLAY_NAMES as ReadonlyArray<string>).includes(scope)
}

// Which consumer the palette opens on. `home` is the general command interface: the commands the app
// answers for, and the tasks a query finds. `search` is the task search alone.
export type PaletteTarget = "home" | "search"

export type KeySpec = string | ReadonlyArray<string>

export interface OverlayControls {
  readonly open: () => void
  readonly close: () => void
  readonly toggle: () => void
}

export interface CursorControls {
  readonly moveBy: (delta: number) => void
  // The path of the row the cursor is on, which is also where opening it goes.
  readonly focusedRow: () => string | undefined
}

export interface PaletteControls {
  readonly open: (target: PaletteTarget) => void
}

export interface CopyControls {
  readonly taskId: () => void
}

export interface DetailControls {
  readonly showFlow: () => void
  readonly editParent: () => void
  readonly addSubtask: () => void
  readonly editTags: () => void
  readonly goToParent: () => void
  readonly escape: () => void
}

export interface RunContext {
  readonly navigate: (to: string) => void
  readonly overlay: (name: OverlayName) => OverlayControls
  readonly palette: PaletteControls
  readonly cursor: CursorControls
  readonly copy: CopyControls
  readonly detail: DetailControls
}

export interface Binding {
  readonly id: string
  readonly keys: KeySpec
  readonly scope: Scope
  readonly when?: (ctx: RunContext) => boolean
  readonly label: string
  readonly group: string
  readonly run: (ctx: RunContext) => void
}
