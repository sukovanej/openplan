import { Schema } from "effect"

export const ChangeEvent = Schema.Union([
  Schema.Struct({
    kind: Schema.Literal("task_changed"),
    id: Schema.String,
    branch: Schema.String,
  }),
  Schema.Struct({
    kind: Schema.Literal("ref_moved"),
    branch: Schema.String,
  }),
  Schema.Struct({
    kind: Schema.Literal("presence_changed"),
    task_id: Schema.String,
  }),
  Schema.Struct({
    kind: Schema.Literal("config_changed"),
  }),
  Schema.Struct({
    kind: Schema.Literal("daemon_stopping"),
  }),
])
export type ChangeEvent = typeof ChangeEvent.Type

export interface Invalidator {
  readonly refreshConfig: () => void
  readonly refreshList: () => void
  readonly refreshTask: (id: string) => void
  readonly refreshVisible: () => void
}

export function applyChange(inv: Invalidator, event: ChangeEvent): void {
  switch (event.kind) {
    case "task_changed": {
      inv.refreshTask(event.id)
      inv.refreshList()
      return
    }
    // A ref move (e.g. `oplan set`) carries no task id, so refetch everything on screen —
    // the open task detail as well as the list.
    case "ref_moved": {
      inv.refreshVisible()
      return
    }
    case "presence_changed": {
      inv.refreshList()
      return
    }
    // The store's key prefix changed, so every id on screen is spelled differently: re-read the
    // config, then everything showing an id.
    case "config_changed": {
      inv.refreshConfig()
      inv.refreshVisible()
      return
    }
    // A connection-lifecycle signal, not a data change: the realtime layer handles it.
    case "daemon_stopping": {
      return
    }
  }
}
