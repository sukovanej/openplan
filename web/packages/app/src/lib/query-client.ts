import { MutationCache, QueryClient } from "@tanstack/react-query"

import type { Invalidator } from "./events"

interface ProjectMutationMeta extends Record<string, unknown> {
  readonly project: string
}

declare module "@tanstack/react-query" {
  interface Register {
    mutationMeta: ProjectMutationMeta
  }
}

export const projectKey = (project: string) => ["project", project] as const
export const projectsKey = ["projects"] as const
export const mergedKey = ["merged"] as const
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

const mutationCache = new MutationCache({
  onSettled: (_data, _error, _variables, _result, mutation) => {
    if (mutation.meta === undefined) return
    void queryClient.invalidateQueries({ queryKey: projectKey(mutation.meta.project) })
    void queryClient.invalidateQueries({ queryKey: mergedKey })
  },
})

export const queryClient = new QueryClient({
  mutationCache,
  defaultOptions: {
    queries: {
      staleTime: Infinity,
      retry: false,
      refetchOnWindowFocus: false,
      refetchOnReconnect: false,
    },
    mutations: {
      retry: false,
    },
  },
})

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
