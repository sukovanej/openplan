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
    kind: Schema.Literal("tags_changed"),
    project: Schema.String,
    branch: Schema.String,
  }),
  Schema.Struct({
    kind: Schema.Literal("projects_changed"),
  }),
  Schema.Struct({
    kind: Schema.Literal("rolling_updates_changed"),
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
  readonly refreshRollingUpdates: (project: string) => void
  // Everything on screen that a change in `project` can have changed, or — with no project — every
  // read there is.
  readonly refreshVisible: (project?: string) => void
}

export function applyChange(inv: Invalidator, event: ChangeEvent): void {
  switch (event.kind) {
    case "task_changed": {
      inv.refreshTask(event.project, event.id)
      inv.refreshList(event.project)
      inv.refreshRollingUpdates(event.project)
      return
    }
    // A ref move (e.g. `openplan set`) carries no task id, so refetch everything on screen —
    // the open task detail as well as the list.
    case "ref_moved": {
      inv.refreshVisible(event.project)
      return
    }
    case "presence_changed": {
      inv.refreshList(event.project)
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
    // The branch committed, rebased, published, or stopped at a conflict. All four change what the
    // header control reads, and none of them changes a task the aggregation returns.
    case "rolling_updates_changed": {
      inv.refreshRollingUpdates(event.project)
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
