import { Schema } from "effect"

export const ChangeEvent = Schema.Union([
  Schema.Struct({
    kind: Schema.Literal("task_changed"),
    project: Schema.String,
    id: Schema.String,
  }),
  Schema.Struct({
    kind: Schema.Literal("tags_changed"),
    project: Schema.String,
  }),
  Schema.Struct({
    kind: Schema.Literal("projects_changed"),
  }),
  Schema.Struct({
    kind: Schema.Literal("sync_changed"),
    project: Schema.String,
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
  readonly refreshProjects: () => void
  readonly refreshList: (project: string) => void
  readonly refreshTask: (project: string, id: string) => void
  readonly refreshHistory: (project: string) => void
  readonly refreshSync: (project: string) => void
  // Everything on screen that a change in `project` can have changed, or — with no project — every
  // read there is.
  readonly refreshVisible: (project?: string) => void
}

export function applyChange(inv: Invalidator, event: ChangeEvent): void {
  switch (event.kind) {
    // Created, edited, commented on, renumbered, or deleted — by this daemon, or by a sync that
    // brought the change in. Every change is a new revision, so the project's activity moves too.
    case "task_changed": {
      inv.refreshTask(event.project, event.id)
      inv.refreshList(event.project)
      inv.refreshHistory(event.project)
      return
    }
    // A tag was registered, recolored, re-described, renamed, or deleted. A rename rewrites the
    // tags of the tasks that reference it, so every row in the project can read differently.
    case "tags_changed": {
      inv.refreshVisible(event.project)
      return
    }
    // Membership, a rename, a status change, or a new key prefix. The last of them spells every id
    // on screen, and the event names no project, so the list is re-read along with every read there
    // is.
    case "projects_changed": {
      inv.refreshProjects()
      inv.refreshVisible()
      return
    }
    // A sync ran, whether it moved anything or failed. What it brought in arrives as task changes of
    // its own, so only the sync state is re-read.
    case "sync_changed": {
      inv.refreshSync(event.project)
      return
    }
    // The stream dropped events and cannot say which, so nothing on screen can be trusted.
    case "resync": {
      inv.refreshProjects()
      inv.refreshVisible()
      return
    }
    // A connection-lifecycle signal, not a data change: the realtime layer handles it.
    case "daemon_stopping": {
      return
    }
  }
}
