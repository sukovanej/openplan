import { useMutation, useQueries, useQueryClient } from "@tanstack/react-query"

import type { Conflict, MatrixCell, Published } from "@openplan/api-client"

import { getRollingUpdates, publishRollingUpdates } from "./api"
import { useProjects } from "./projects"
import { mergedKey, projectKey, projectMutationsKey, rollingUpdatesKey } from "./query-client"
import { runtime } from "./runtime"

export type SyncState = "offline" | "syncing" | "blocked" | "pending" | "idle"

export interface ProjectUpdates {
  readonly project: string
  readonly pending: ReadonlyArray<MatrixCell>
  readonly conflict: Conflict | undefined
}

export function pendingCount(updates: ReadonlyArray<ProjectUpdates>): number {
  return updates.reduce((total, one) => total + one.pending.length, 0)
}

export function conflicted(updates: ReadonlyArray<ProjectUpdates>): ReadonlyArray<ProjectUpdates> {
  return updates.filter((one) => one.conflict !== undefined)
}

// A conflict outranks a pending count, because it is what stops that count from ever being published.
export function syncState(updates: ReadonlyArray<ProjectUpdates>, live: boolean, publishing: boolean): SyncState {
  if (!live) return "offline"
  if (publishing) return "syncing"
  if (conflicted(updates).length > 0) return "blocked"
  return pendingCount(updates) > 0 ? "pending" : "idle"
}

// A project whose repository cannot host the branch has nothing to publish and no route to ask, so
// it never reaches the control.
export function useRollingUpdates(): ReadonlyArray<ProjectUpdates> {
  const projects = (useProjects() ?? []).filter(
    (project) => project.status.state === "ok" && project.rolling_updates_branch != null,
  )
  return useQueries({
    queries: projects.map((project) => ({
      queryKey: rollingUpdatesKey(project.name),
      queryFn: () => runtime.runPromise(getRollingUpdates(project.name)),
    })),
    combine: (results) =>
      results.flatMap((result, at) => {
        const project = projects[at]
        if (project === undefined || result.data === undefined) return []
        return [{ project: project.name, pending: result.data.pending, conflict: result.data.conflict ?? undefined }]
      }),
  })
}

// Keyed like every other project write, so a refusal reaches the same error toast.
export function usePublish() {
  const client = useQueryClient()
  return useMutation({
    mutationKey: projectMutationsKey,
    mutationFn: (project: string): Promise<Published> => runtime.runPromise(publishRollingUpdates(project)),
    onSettled: (_published, _error, project) =>
      Promise.all([
        client.invalidateQueries({ queryKey: projectKey(project) }),
        client.invalidateQueries({ queryKey: mergedKey }),
      ]),
  })
}
