export type Scope = "global" | "list" | "detail" | "overlay"
export type RouteScope = "list" | "detail"

export type KeySpec = string | ReadonlyArray<string>

export interface OverlayControls {
  readonly open: () => void
  readonly close: () => void
  readonly toggle: () => void
}

export interface CursorControls {
  readonly moveBy: (delta: number) => void
  readonly focusedId: () => string | undefined
}

export interface RunContext {
  readonly navigate: (to: string) => void
  readonly overlay: OverlayControls
  readonly cursor: CursorControls
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
