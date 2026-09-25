import { QueryClient, useMutation, useQueryClient } from "@tanstack/react-query"
import { Effect } from "effect"
import type { HttpClient } from "effect/unstable/http"

import type { Invalidator } from "./events"
import { runtime } from "./runtime"

export type Write = Effect.Effect<unknown, unknown, HttpClient.HttpClient>

export const projectKey = (project: string) => ["project", project] as const
export const projectsKey = ["projects"] as const
export const mergedKey = ["merged"] as const
export const projectMutationsKey = ["mutation", "project"] as const
export const mergedBoardKey = [...mergedKey, "board"] as const
// The flow spans every project a query names, so it lives beside the merged board rather than under
// one project.
export const flowsKey = [...mergedKey, "flow"] as const
export const flowKey = (query: string) => [...flowsKey, query] as const
export const boardKey = (project: string) => [...projectKey(project), "board"] as const
export const tasksKey = (project: string) => [...projectKey(project), "tasks"] as const
export const tagsKey = (project: string) => [...projectKey(project), "tags"] as const
export const syncKey = (project: string) => [...projectKey(project), "sync"] as const
export const historyKey = (project: string) => [...projectKey(project), "history"] as const
export const taskKey = (project: string, id: string) => [...projectKey(project), "task", id] as const
// Under the task, so whatever re-reads the task re-reads the revisions that changed it.
export const taskHistoryKey = (project: string, id: string) => [...taskKey(project, id), "history"] as const
// Apart from the task, because a revision never changes: a change to the task leaves it as it was.
export const revisionKey = (project: string, id: string, revision: string) =>
  [...projectKey(project), "revision", id, revision] as const

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: Infinity,
      retry: false,
      refetchOnWindowFocus: false,
      refetchOnReconnect: false,
    },
  },
})

export function useProjectMutation(project: string) {
  const client = useQueryClient()
  return useMutation({
    mutationKey: [...projectMutationsKey, project],
    mutationFn: (effect: Write) => runtime.runPromise(effect),
    onSettled: () =>
      Promise.all([
        client.invalidateQueries({ queryKey: projectKey(project) }),
        client.invalidateQueries({ queryKey: mergedKey }),
      ]),
  })
}

const invalidate = (queryKey: ReadonlyArray<unknown>) => {
  void queryClient.invalidateQueries({ queryKey })
}

export const queryInvalidator: Invalidator = {
  refreshProjects: () => invalidate(projectsKey),
  refreshList: (project) => {
    invalidate(boardKey(project))
    invalidate(tasksKey(project))
    invalidate(mergedBoardKey)
    invalidate(flowsKey)
  },
  refreshTask: (project, id) => invalidate(taskKey(project, id)),
  refreshHistory: (project) => invalidate(historyKey(project)),
  refreshSync: (project) => invalidate(syncKey(project)),
  refreshVisible: (project) => {
    if (project === undefined) {
      invalidate(["project"])
    } else {
      invalidate(projectKey(project))
    }
    invalidate(mergedKey)
  },
}
