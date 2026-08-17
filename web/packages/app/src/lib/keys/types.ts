export type Scope = "global" | "list" | "detail" | "overlay" | "rows"
export type RouteScope = "list" | "detail"

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

export interface CopyControls {
  readonly taskId: () => void
}

export interface DetailControls {
  readonly editParent: () => void
  readonly addSubtask: () => void
  readonly goToParent: () => void
  readonly escape: () => void
}

export interface RunContext {
  readonly navigate: (to: string) => void
  readonly overlay: OverlayControls
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
