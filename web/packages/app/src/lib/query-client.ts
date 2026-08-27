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
export const boardKey = (project: string) => [...projectKey(project), "board"] as const
export const tasksKey = (project: string) => [...projectKey(project), "tasks"] as const
export const tagsKey = (project: string, branch?: string) =>
  branch === undefined
    ? ([...projectKey(project), "tags"] as const)
    : ([...projectKey(project), "tags", branch] as const)
export const taskKey = (project: string, id: string, branch?: string) =>
  branch === undefined
    ? ([...projectKey(project), "task", id] as const)
    : ([...projectKey(project), "task", id, branch] as const)

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
  },
  refreshTask: (project, id) => invalidate(taskKey(project, id)),
  refreshVisible: (project) => {
    if (project === undefined) {
      invalidate(["project"])
    } else {
      invalidate(projectKey(project))
    }
    invalidate(mergedKey)
  },
}
