import { Schema } from "effect"

export const ChangeEvent = Schema.Union([
  Schema.Struct({
    kind: Schema.Literal("task_changed"),
    project: Schema.String,
    id: Schema.String,
    branch: Schema.String,
  }),
  Schema.Struct({
    kind: Schema.Literal("ref_moved"),
    project: Schema.String,
    branch: Schema.String,
  }),
  Schema.Struct({
    kind: Schema.Literal("presence_changed"),
    project: Schema.String,
    task_id: Schema.String,
  }),
  Schema.Struct({
    kind: Schema.Literal("projects_changed"),
  }),
  Schema.Struct({
    kind: Schema.Literal("resync"),
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
    // Membership, a rename, a status change, or a new key prefix. The last of them spells every id
    // on screen, so the config is re-read along with everything showing one.
    case "projects_changed": {
      inv.refreshConfig()
      inv.refreshVisible()
      return
    }
    // The stream dropped events and cannot say which, so nothing on screen can be trusted.
    case "resync": {
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
