// The revisions of one task take one history entry between them, so Back and Esc leave them for the
// page that opened the first one. A revision opened from the live task sits on the entry just above
// it, and says so in its state, so the way back to the current version can be a Back.
const OVER_LIVE_TASK = { overLiveTask: true } as const

export function isOverLiveTask(state: unknown): boolean {
  return (state as { overLiveTask?: unknown } | null)?.overLiveTask === true
}

export interface RevisionNavigation {
  readonly replace: boolean
  readonly state: unknown
}

export function revisionNavigation(onRevision: boolean, state: unknown): RevisionNavigation {
  return onRevision ? { replace: true, state } : { replace: false, state: OVER_LIVE_TASK }
}
