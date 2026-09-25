import { useMutation, useQueries, useQueryClient } from "@tanstack/react-query"

import type { Board, ProjectView, SyncResult, SyncView, TaskListItem } from "@openplan/api-client"

import { getSync, runSync } from "./api"
import { useProjects } from "./projects"
import { projectMutationsKey, syncKey } from "./query-client"
import { abortable, runtime } from "./runtime"

export type SyncState = "offline" | "syncing" | "failed" | "waiting" | "idle"

export interface ProjectSync {
  readonly project: string
  readonly view: SyncView
}

export function waitingCount(syncs: ReadonlyArray<ProjectSync>): number {
  return syncs.reduce((total, one) => total + one.view.ahead + one.view.behind, 0)
}

export function failed(syncs: ReadonlyArray<ProjectSync>): ReadonlyArray<ProjectSync> {
  return syncs.filter((one) => one.view.error !== undefined)
}

export function tasksInConflict(board: Board): ReadonlyArray<TaskListItem> {
  return board.groups.flatMap((group) => group.rows.map((row) => row.task)).filter((task) => task.conflicts > 0)
}

// A failure outranks a count, because it is what keeps the count from going down.
export function syncState(syncs: ReadonlyArray<ProjectSync>, live: boolean, working: boolean): SyncState {
  if (!live) return "offline"
  if (working) return "syncing"
  if (failed(syncs).length > 0) return "failed"
  return waitingCount(syncs) > 0 ? "waiting" : "idle"
}

const revisions = (count: number): string => `${count} ${count === 1 ? "revision" : "revisions"}`

export function syncResultText(result: SyncResult): string {
  if (result.received === 0 && result.sent === 0) return "Nothing to receive or send."
  const parts: Array<string> = []
  if (result.received > 0) parts.push(`Received ${revisions(result.received)}.`)
  if (result.merged) parts.push("Merged them with the local changes.")
  if (result.sent > 0) parts.push(`Sent ${revisions(result.sent)}.`)
  return parts.join(" ")
}

// Only tasks in a git ref sync, and only with a remote to sync with. The daemon names that remote
// in `sync`, so a local project and a repository without a remote both come without one.
export function syncable(project: ProjectView): boolean {
  return project.status.state === "ok" && project.backend === "git" && project.sync !== undefined
}

// The project list already carries each state, so the first read costs nothing. A sync event re-reads
// the one project it names.
export function useProjectSyncs(): ReadonlyArray<ProjectSync> {
  const projects = (useProjects() ?? []).filter(syncable)
  return useQueries({
    queries: projects.map((project) => ({
      queryKey: syncKey(project.name),
      queryFn: abortable(getSync(project.name)),
      initialData: project.sync,
    })),
    combine: (results) =>
      results.flatMap((result, at) => {
        const project = projects[at]
        if (project === undefined || result.data === undefined) return []
        return [{ project: project.name, view: result.data }]
      }),
  })
}

// Keyed like every other project write, so a refusal reaches the same error toast.
export function useSyncNow() {
  const client = useQueryClient()
  return useMutation({
    mutationKey: projectMutationsKey,
    mutationFn: (project: string): Promise<SyncResult> => runtime.runPromise(runSync(project)),
    onSuccess: (result, project) => client.setQueryData(syncKey(project), result.status),
    // A sync that failed records why, and the answer that carried the failure did not carry that.
    onError: (_error, project) => client.invalidateQueries({ queryKey: syncKey(project) }),
  })
}
